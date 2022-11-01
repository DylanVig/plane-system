use anyhow::Context;
use clap::Parser;
use ctrlc;
use rustyline_async::{Readline, SharedWriter};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt, Layer};

use crate::cli::interactive::run_interactive_cli;

#[macro_use]
extern crate tracing;

mod cli;
mod config;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    // setup colorful backtraces
    color_backtrace::install();

    // set up logging and interactive line editor
    let (editor, stdout) =
        Readline::new("ps> ".into()).context("failed to create interactive editor")?;

    let mut logging_unset_warning = false;
    let mut logging_targets = tracing_subscriber::filter::Targets::new();

    if let Ok(directives) = std::env::var("RUST_LOG") {
        for directive in directives.split(',') {
            if let Some((target, level)) = directive.split_once('=') {
                logging_targets = logging_targets.with_target(
                    target,
                    level.parse::<LevelFilter>().context("invalid log level")?,
                );
            } else {
                logging_targets = logging_targets.with_default(
                    directive
                        .parse::<LevelFilter>()
                        .context("invalid log level")?,
                );
            }
        }
    } else {
        logging_targets = logging_targets.with_target("plane_system", LevelFilter::INFO);
        logging_unset_warning = true;
    }

    let (writer, _guard) =
        tracing_appender::non_blocking(tracing_appender::rolling::hourly("logs", "plane-system"));

    let reg = tracing_subscriber::registry();

    #[cfg(tokio_unstable)]
    let reg = reg.with(console_subscriber::spawn());

    reg
        // writer that outputs to console
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer({
                    let stdout = stdout.clone();
                    move || stdout.clone()
                })
                .with_filter(logging_targets),
        )
        // writer that outputs to files
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(writer)
                .with_filter(Targets::new().with_target("plane_system", LevelFilter::DEBUG)),
        )
        .init();

    let mut features = vec![];

    #[cfg(feature = "aux-camera")]
    features.push("aux-camera");

    #[cfg(feature = "csb")]
    features.push("csb");

    info!(
        "initializing plane system v{} ({})",
        env!("CARGO_PKG_VERSION"),
        features.join(",")
    );

    if logging_unset_warning {
        warn!("RUST_LOG environment variable was not specified, so only logs from the plane system w/ level INFO or higher will be shown! specifying RUST_LOG is strongly recommended");
    }

    let main_args: cli::args::MainArgs = cli::args::MainArgs::parse();

    debug!("reading config from {:?}", &main_args.config);
    let config = crate::config::PlaneSystemConfig::read_from_path(&main_args.config)
        .context("failed to read config file")?;
    let config = config;

    run_tasks(config, editor, stdout).await
}

async fn run_tasks(
    config: crate::config::PlaneSystemConfig,
    editor: Readline,
    stdout: SharedWriter,
) -> anyhow::Result<()> {
    let cancellation_token = CancellationToken::new();

    ctrlc::set_handler({
        let cancellation_token = cancellation_token.clone();
        move || {
            info!("received interrupt, shutting down");
            cancellation_token.cancel();
        }
    })
    .expect("could not set ctrl+c handler");

    let mut tasks = Vec::<Box<dyn ps_client::Task + Send>>::new();

    let pixhawk_evt_rx = match config.pixhawk {
        Some(c) => {
            debug!("initializing pixhawk task");
            let evt_task =
                ps_pixhawk::create_task(c).context("failed to initialize pixhawk task")?;
            let pixhawk_evt_rx = evt_task.events();
            tasks.push(Box::new(evt_task));
            Some(pixhawk_evt_rx)
        }
        None => None,
    };

    debug!("initializing telemetry task");
    let telem_task = ps_telemetry::create_task(pixhawk_evt_rx, None)
        .context("failed to initialize telemetry task")?;
    let telem_rx = telem_task.telemetry();
    tasks.push(Box::new(telem_task));

    let camera_cmd_tx = if let Some(c) = config.main_camera {
        debug!("initializing camera tasks");
        let (control_task, evt_task, download_task) =
            ps_main_camera::create_tasks(c).context("failed to initialize camera tasks")?;

        let camera_cmd_tx = control_task.cmd();
        let camera_download_rx = download_task.download();

        tasks.push(Box::new(control_task));
        tasks.push(Box::new(evt_task));
        tasks.push(Box::new(download_task));

        if let Some(c) = config.download {
            debug!("initializing download task");
            let download_task = ps_download::create_task(c, telem_rx, camera_download_rx)
                .context("failed to initialize download task")?;
            tasks.push(Box::new(download_task));
        }

        Some(camera_cmd_tx)
    } else {
        None
    };

    #[cfg(feature = "aux-camera")]
    let aux_camera_save_cmd_tx = if let Some(c) = config.aux_camera {
        debug!("initializing aux camera tasks");

        let (stream_task, save_task) = ps_aux_camera::create_tasks(c)?;

        let mut aux_camera_save_cmd_tx = None;

        if let Some(stream_task) = stream_task {
            tasks.push(Box::new(stream_task));
        }
        if let Some(save_task) = save_task {
            aux_camera_save_cmd_tx = Some(save_task.cmd());
            tasks.push(Box::new(save_task));
        }

        aux_camera_save_cmd_tx
    } else {
        None
    };

    #[cfg(not(feature = "aux-camera"))]
    let aux_camera_save_cmd_tx = None;

    // TODO: gs task

    let mut join_set = JoinSet::new();

    join_set.spawn(run_interactive_cli(
        editor,
        stdout,
        camera_cmd_tx,
        aux_camera_save_cmd_tx,
        cancellation_token.clone(),
    ));

    for task in tasks {
        let task_name = task.name();
        debug!("starting {} task", task_name);
        let ct = cancellation_token.clone();

        join_set.spawn(async move {
            // drop guard is there to print a log message when the future is
            // dropped, which happens when the task terminates for any reason
            let _dg = drop_guard::guard((), |_| debug!("exiting {} task", task_name));

            task.run(ct).await
        });
    }

    while let Some(res) = join_set.join_next().await {
        // if task panicked, then will be Some(Err)
        // if task terminated w/ error, then will be Some(Ok(Err))
        // need to propagate errors in both cases

        let ctdg = cancellation_token.clone().drop_guard();

        res.context("task failed")?
            .context("task terminated with error")?;

        // if task exited w/o any sort of error, don't trigger cancellation of
        // other tasks
        ctdg.disarm();
    }

    Ok(())
}
