use anyhow::Context;
use clap::Parser;
use ctrlc;
use rustyline_async::{Readline, SharedWriter};
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt, Layer};

use crate::cli::interactive::{run_interactive_cli, CliChannels};

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

    let (writer, writer_guard) =
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
                .with_filter(Targets::new().with_targets(vec![
                    ("plane_system", LevelFilter::DEBUG),
                    ("ps_livestream", LevelFilter::DEBUG),
                    ("ps_main_camera", LevelFilter::DEBUG),
                    ("ps_main_camera_csb", LevelFilter::TRACE),
                    ("ps_telemetry", LevelFilter::DEBUG),
                    ("ps_gs", LevelFilter::DEBUG),
                    ("ps_pixhawk", LevelFilter::DEBUG),
                    ("ps_gimbal", LevelFilter::DEBUG),
                ])),
        )
        .init();

    let mut features = vec![];

    #[cfg(feature = "livestream")]
    features.push("livestream");

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

    let result = run_tasks(config, editor, stdout).await;

    if let Err(err) = &result {
        error!("program exited with error: {err:?}");
    }

    std::mem::drop(writer_guard);

    result
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
                ps_pixhawk::create_tasks(c).context("failed to initialize pixhawk task")?;
            let pixhawk_evt_rx = evt_task.events();
            tasks.push(Box::new(evt_task));
            Some(pixhawk_evt_rx)
        }
        None => None,
    };

    #[cfg(feature = "csb")]
    let csb_evt_rx = if let Some(camera_config) = &config.main_camera {
        if let Some(c) = &camera_config.current_sensing {
            debug!("initializing csb task");

            let (evt_task, csb_rx) = ps_main_camera::csb::create_task(c.clone())
                .context("failed to initialize csb task")?;

            tasks.push(Box::new(evt_task));

            Some(csb_rx)
        } else {
            None
        }
    } else {
        None
    };

    debug!("initializing telemetry task");
    let telem_task = ps_telemetry::create_task(pixhawk_evt_rx, csb_evt_rx)
        .context("failed to initialize telemetry task")?;
    let telem_rx = telem_task.telemetry();
    tasks.push(Box::new(telem_task));

    let gs_cmd_tx = if let Some(c) = config.ground_server {
        debug!("initializing ground server tasks");

        let upload_task = ps_gs::create_task(c)?;

        let cmd_tx = upload_task.cmd();

        tasks.push(Box::new(upload_task));

        Some(cmd_tx)
    } else {
        None
    };

    let (camera_ctrl_cmd_tx, camera_preview_frame_rx) = if let Some(c) = config.main_camera {
        debug!("initializing camera tasks");
        let (control_task, evt_task, download_task, live_task) =
            ps_main_camera::create_tasks(c, telem_rx, gs_cmd_tx)
                .context("failed to initialize camera tasks")?;

        let ctrl_cmd_tx = control_task.cmd();
        let mut preview_frame_rx = None;

        tasks.push(Box::new(control_task));
        tasks.push(Box::new(evt_task));
        tasks.push(Box::new(download_task));

        if let Some(live_task) = live_task {
            preview_frame_rx = Some(live_task.frame());
            tasks.push(Box::new(live_task));
        }

        (Some(ctrl_cmd_tx), preview_frame_rx)
    } else {
        (None, None)
    };

    #[cfg(feature = "livestream")]
    let livestream_cmd_tx = if let Some(c) = config.livestream {
        debug!("initializing aux camera tasks");

        let (custom_task, preview_task) = ps_livestream::create_tasks(c, camera_preview_frame_rx)?;

        let mut livestream_cmd_tx = None;

        if let Some(custom_task) = custom_task {
            livestream_cmd_tx = Some(custom_task.cmd());
            tasks.push(Box::new(custom_task));
        }

        if let Some(preview_task) = preview_task {
            tasks.push(Box::new(preview_task));
        }

        livestream_cmd_tx
    } else {
        None
    };

    #[cfg(not(feature = "livestream"))]
    let livestream_save_cmd_tx = None;

    let gimbal_cmd_tx = if let Some(c) = config.gimbal {
        debug!("initializing gimbal task");
        let gimbal_task = ps_gimbal::create_task(c)?;

        let gimbal_cmd_tx = gimbal_task.cmd();
        tasks.push(Box::new(gimbal_task));

        Some(gimbal_cmd_tx)
    } else {
        None
    };

    let mut join_set = JoinSet::new();

    let cli_channels = CliChannels {
        camera_cmd_tx: camera_ctrl_cmd_tx,
        livestream_cmd_tx,
        gimbal_cmd_tx,
    };

    join_set.spawn(run_interactive_cli(
        editor,
        stdout,
        cli_channels,
        cancellation_token.clone(),
    ));

    for task in tasks {
        let task_name = task.name();
        debug!("starting {} task", task_name);
        let ct = cancellation_token.clone();

        let fut = async move {
            // drop guard is there to print a log message when the future is
            // dropped, which happens when the task terminates for any reason
            let _dg = drop_guard::guard((), |_| debug!("exiting {} task", task_name));

            task.run(ct).await
        };

        #[cfg(tokio_unstable)]
        join_set
            .build_task()
            .name(task_name)
            .spawn(fut)
            .context("failed to spawn future")?;

        #[cfg(not(tokio_unstable))]
        join_set.spawn(fut);
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
