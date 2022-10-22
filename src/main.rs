use std::{process::exit, sync::Arc, time::Duration};

use anyhow::Context;
use clap::Parser;
use ctrlc;
use futures::{channel::oneshot, Future};
use tokio::{
    sync::{broadcast, watch},
    task::JoinHandle,
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use tracing::metadata::LevelFilter;
use tracing_subscriber::{filter::Targets, layer::SubscriberExt, util::SubscriberInitExt, Layer};

#[macro_use]
extern crate tracing;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate num_derive;
#[macro_use]
extern crate async_trait;

mod cli;

struct TaskBag {
    names: Vec<String>,
    tasks: Vec<JoinHandle<anyhow::Result<()>>>,
}

impl TaskBag {
    pub fn new() -> Self {
        Self {
            names: vec![],
            tasks: vec![],
        }
    }

    pub fn add(
        &mut self,
        name: &str,
        task: impl Future<Output = anyhow::Result<()>> + Send + 'static,
    ) {
        info!("spawning task \"{}\"", name);
        self.names.push(name.to_owned());

        #[cfg(tokio_unstable)]
        self.tasks
            .push(tokio::task::Builder::new().name(name).spawn(task).unwrap());

        #[cfg(not(tokio_unstable))]
        self.tasks.push(tokio::spawn(task));
    }

    pub async fn wait(&mut self) -> anyhow::Result<()> {
        while self.tasks.len() > 0 {
            let tasks = std::mem::replace(&mut self.tasks, vec![]);

            // wait for each task to end
            let (result, i, remaining) = futures::future::select_all(tasks).await;
            let name = self.names.remove(i);

            if self.names.len() > 0 {
                let names_quoted = self
                    .names
                    .iter()
                    .map(|n| format!("\"{}\"", n))
                    .collect::<Vec<_>>()
                    .join(", ");

                info!("task \"{}\" ended, tasks {} remaining", name, names_quoted);
            } else {
                info!("task \"{}\" ended, no tasks remaining", name);
            }

            // if a task ended with an error, end the process with an interrupt
            if let Err(err) = result.unwrap() {
                error!("got error from task \"{}\": {:?}", name, err);
                return Err(err);
            }

            self.tasks = remaining;
        }

        Ok(())
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    color_backtrace::install();

    let mut targets = tracing_subscriber::filter::Targets::new();

    if let Ok(directives) = std::env::var("RUST_LOG") {
        for directive in directives.split(',') {
            if let Some((target, level)) = directive.split_once('=') {
                targets = targets.with_target(
                    target,
                    level.parse::<LevelFilter>().context("invalid log level")?,
                );
            } else {
                targets = targets.with_default(
                    directive
                        .parse::<LevelFilter>()
                        .context("invalid log level")?,
                );
            }
        }
    }

    let (writer, _guard) =
        tracing_appender::non_blocking(tracing_appender::rolling::hourly("logs", "plane-system"));

    let reg = tracing_subscriber::registry();

    #[cfg(tokio_unstable)]
    let reg = reg.with(console_subscriber::spawn());

    reg
        // writer that outputs to console
        .with(tracing_subscriber::fmt::layer().with_filter(targets))
        // writer that outputs to files
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(writer)
                .with_filter(
                    Targets::new().with_targets(vec![("plane_system", LevelFilter::DEBUG)]),
                ),
        )
        .init();

    let main_args: cli::args::MainArgs = cli::args::MainArgs::parse();

    let config = if let Some(config_path) = main_args.config {
        debug!("reading config from {:?}", &config_path);
        cli::config::PlaneSystemConfig::read_from_path(config_path)
    } else {
        debug!("reading config from default location");
        cli::config::PlaneSystemConfig::read()
    };

    let config = config.context("failed to read config file")?;

    run_tasks(config).await
}

async fn run_tasks(config: cli::config::PlaneSystemConfig) -> anyhow::Result<()> {
    let cancellation_token = CancellationToken::new();

    ctrlc::set_handler({
        let interrupt_sender = interrupt_sender.clone();
        move || {
            info!("received interrupt, shutting down");
            cancellation_token.cancel();
        }
    })
    .expect("could not set ctrl+c handler");

    let mut tasks = TaskBag::new();

    {
        let (pixhawk_telemetry_sender, pixhawk_telemetry_receiver) = watch::channel(None);
        let (pixhawk_event_sender, _) = broadcast::channel(64);
        let (camera_event_sender, _) = broadcast::channel(256);
        #[cfg(feature = "csb")]
        let (csb_telemetry_sender, csb_telemetry_receiver) = watch::channel(None);
        let (camera_cmd_sender, camera_cmd_receiver) = flume::unbounded();
        let (gimbal_cmd_sender, _gimbal_cmd_receiver) = flume::unbounded();
        let (scheduler_cmd_sender, scheduler_cmd_receiver) = flume::unbounded();
        #[cfg(feature = "gstreamer")]
        let (stream_cmd_sender, stream_cmd_receiver) = flume::unbounded();
        #[cfg(feature = "gstreamer")]
        let (save_cmd_sender, save_cmd_receiver) = flume::unbounded();
        let (image_event_sender, _) = broadcast::channel(256);
        let (pixhawk_cmd_sender, pixhawk_cmd_receiver) = flume::unbounded();

        let channels = Arc::new(Channels {
            interrupt: interrupt_sender.clone(),
            pixhawk_telemetry: pixhawk_telemetry_receiver,
            pixhawk_event: pixhawk_event_sender,
            pixhawk_cmd: pixhawk_cmd_sender,
            camera_event: camera_event_sender,
            #[cfg(feature = "csb")]
            csb_telemetry: csb_telemetry_receiver,
            camera_cmd: camera_cmd_sender,
            gimbal_cmd: gimbal_cmd_sender,
            #[cfg(feature = "gstreamer")]
            stream_cmd: stream_cmd_sender,
            #[cfg(feature = "gstreamer")]
            save_cmd: save_cmd_sender,
            image_event: image_event_sender,
            scheduler_cmd: scheduler_cmd_sender,
        });

        if let Some(pixhawk_config) = config.0 {
            tasks.add("pixhawk", {
                let channels = channels.clone();
                async move {
                    let pixhawk_client = PixhawkClient::connect(
                        channels.clone(),
                        pixhawk_cmd_receiver,
                        pixhawk_config.address,
                        pixhawk_config.mavlink,
                    )
                    .await?;
                    pixhawk_client.run().await
                }
            });

            tasks.add("telemetry", {
                let telemetry = TelemetryStream::new(channels.clone(), pixhawk_telemetry_sender);
                async move { telemetry.run().await }
            });
        } else {
            info!(
                "pixhawk address not specified, disabling pixhawk connection and telemetry stream"
            );
        }

        if let Some(camera_config) = config.3 {
            tasks.add("camera", {
                camera::main::run(channels.clone(), camera_cmd_receiver)
            });

            #[cfg(feature = "csb")]
            if let Some(csb_config) = camera_config.current_sensing {
                tasks.add("current sensing", {
                    camera::main::csb::run(channels.clone(), csb_telemetry_sender, csb_config)
                });
            }
        }

        if let Some(image_config) = config.2 {
            tasks.add("image download", {
                image::run(channels.clone(), image_config)
            });
        }

        if let Some(_gimbal_config) = config.5 {
            panic!("gimbal not implemented");
        }

        if let Some(gs_config) = config.1 {
            tasks.add("ground server", {
                let gs_client = GroundServerClient::new(channels.clone(), gs_config.address)?;
                async move { gs_client.run().await }
            });
        }

        if let Some(_scheduler_config) = config.6 {
            tasks.add("scheduler", {
                scheduler::run(channels.clone(), scheduler_cmd_receiver)
            });
        }

        #[cfg(feature = "gstreamer")]
        if let Some(aux_config) = config.aux_camera {
            if let Some(stream_config) = aux_config.stream {
                tasks.add("aux camera live stream", {
                    let mut stream_client = camera::auxiliary::stream::StreamClient::connect(
                        channels.clone(),
                        stream_cmd_receiver,
                        stream_config.address,
                        aux_config.cameras.clone(),
                    )?;
                    async move { stream_client.run().await }
                });
            }

            if let Some(save_config) = aux_config.save {
                tasks.add("aux camera live record", {
                    let mut save_client = camera::auxiliary::save::SaveClient::connect(
                        channels.clone(),
                        save_cmd_receiver,
                        save_config.save_path,
                        aux_config.cameras.clone(),
                    )?;
                    async move { save_client.run().await }
                });
            }
        }

        tasks.add("command line interface", {
            let channels = channels.clone();
            cli::repl::run(channels)
        });

        tasks.add("plane server", {
            let channels = channels.clone();
            server::serve(channels, config.plane_server.address)
        });
    }

    if let Err(_) = tasks.wait().await {
        let _ = interrupt_sender.send(());

        info!(
            "terminating in 5 seconds, remaining tasks: {}",
            tasks.names.join(", ")
        );
        sleep(Duration::from_secs(5)).await;
        info!(
            "terminating now, remaining tasks: {}",
            tasks.names.join(", ")
        );
        exit(1);
    }

    info!("exit");

    Ok(())
}
