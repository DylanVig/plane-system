use std::{process::exit, sync::Arc, time::Duration};

use anyhow::Context;
use ctrlc;
use futures::{channel::oneshot, Future};
use structopt::StructOpt;
use tokio::{
    spawn,
    sync::{broadcast, watch},
    task::JoinHandle,
    time::sleep,
};

use gimbal::client::GimbalClient;
use gs::GroundServerClient;
use pixhawk::{client::PixhawkClient, state::PixhawkEvent};
use scheduler::Scheduler;
use state::TelemetryInfo;
use telemetry::TelemetryStream;

#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate num_derive;
#[macro_use]
extern crate async_trait;

mod camera;
mod cli;
mod gimbal;
mod gs;
mod image;
mod pixhawk;
mod scheduler;
mod server;
mod state;
mod telemetry;
mod util;

#[derive(Debug)]
pub struct Channels {
    /// Channel for broadcasting a signal when the system should terminate.
    interrupt: broadcast::Sender<()>,

    /// Channel for broadcasting telemetry information gathered from the gimbal and pixhawk
    telemetry: watch::Receiver<Option<TelemetryInfo>>,

    /// Channel for broadcasting updates to the state of the Pixhawk.
    pixhawk_event: broadcast::Sender<PixhawkEvent>,

    /// Channel for sending instructions to the Pixhawk.
    pixhawk_cmd: flume::Sender<pixhawk::PixhawkCommand>,

    /// Channel for broadcasting updates to the state of the camera.
    camera_event: broadcast::Sender<camera::main::CameraClientEvent>,

    /// Channel for sending instructions to the camera.
    camera_cmd: flume::Sender<camera::main::CameraCommand>,

    /// Channel for sending instructions to the gimbal.
    gimbal_cmd: flume::Sender<gimbal::GimbalCommand>,

    ///Channel for starting stream.
    #[cfg(feature = "gstreamer")]
    stream_cmd: flume::Sender<camera::aux::stream::StreamCommand>,

    ///Channel for starting saver.
    #[cfg(feature = "gstreamer")]
    save_cmd: flume::Sender<camera::aux::save::SaveCommand>,

    image_event: broadcast::Sender<image::ImageClientEvent>,
}

#[derive(Debug)]
pub struct Command<Req, Res, Err = anyhow::Error> {
    request: Req,
    chan: oneshot::Sender<Result<Res, Err>>,
}

impl<Req, Res, Err> Command<Req, Res, Err> {
    fn new(request: Req) -> (Self, oneshot::Receiver<Result<Res, Err>>) {
        let (sender, receiver) = oneshot::channel();

        let cmd = Command {
            chan: sender,
            request,
        };

        (cmd, receiver)
    }

    fn channel(self) -> oneshot::Sender<Result<Res, Err>> {
        self.chan
    }

    fn request(&self) -> &Req {
        &self.request
    }

    fn respond(self, result: Result<Res, Err>) -> Result<(), Result<Res, Err>> {
        let chan = self.channel();

        if chan.is_canceled() {
            panic!("chan is closed");
        }

        chan.send(result)
    }
}

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
        info!("spawning task {}", name);
        self.names.push(name.to_owned());
        self.tasks.push(spawn(task));
    }

    pub async fn wait(&mut self) -> anyhow::Result<()> {
        while self.tasks.len() > 0 {
            let tasks = std::mem::replace(&mut self.tasks, vec![]);

            // wait for each task to end
            let (result, i, remaining) = futures::future::select_all(tasks).await;
            let name = self.names.remove(i);

            info!("task {} ended, {} remaining", name, self.names.join(", "));

            // if a task ended with an error, end the process with an interrupt
            if let Err(err) = result.unwrap() {
                error!("got error from task {}: {:?}", name, err);
                return Err(err);
            }

            self.tasks = remaining;
        }

        Ok(())
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init_timed();

    let main_args: cli::args::MainArgs = cli::args::MainArgs::from_args();

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
    let (interrupt_sender, _) = broadcast::channel(1);

    ctrlc::set_handler({
        let interrupt_sender = interrupt_sender.clone();
        move || {
            info!("received interrupt, shutting down");
            let _ = interrupt_sender.send(());
        }
    })
    .expect("could not set ctrl+c handler");

    let mut tasks = TaskBag::new();

    {
        let (telemetry_sender, telemetry_receiver) = watch::channel(None);
        let (pixhawk_event_sender, _) = broadcast::channel(64);
        let (camera_event_sender, _) = broadcast::channel(256);
        let (camera_cmd_sender, camera_cmd_receiver) = flume::unbounded();
        let (gimbal_cmd_sender, gimbal_cmd_receiver) = flume::unbounded();
        #[cfg(feature = "gstreamer")]
        let (stream_cmd_sender, stream_cmd_receiver) = flume::unbounded();
        #[cfg(feature = "gstreamer")]
        let (save_cmd_sender, save_cmd_receiver) = flume::unbounded();
        let (image_event_sender, _) = broadcast::channel(256);
        let (pixhawk_cmd_sender, pixhawk_cmd_receiver) = flume::unbounded();

        let channels = Arc::new(Channels {
            interrupt: interrupt_sender.clone(),
            telemetry: telemetry_receiver,
            pixhawk_event: pixhawk_event_sender,
            pixhawk_cmd: pixhawk_cmd_sender,
            camera_event: camera_event_sender,
            camera_cmd: camera_cmd_sender,
            gimbal_cmd: gimbal_cmd_sender,
            #[cfg(feature = "gstreamer")]
            stream_cmd: stream_cmd_sender,
            #[cfg(feature = "gstreamer")]
            save_cmd: save_cmd_sender,
            image_event: image_event_sender,
        });

        if let Some(pixhawk_config) = config.pixhawk {
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
                let telemetry = TelemetryStream::new(channels.clone(), telemetry_sender);
                async move { telemetry.run().await }
            });
        } else {
            info!(
                "pixhawk address not specified, disabling pixhawk connection and telemetry stream"
            );
        }

        if let Some(camera_config) = config.main_camera {
            tasks.add("camera", {
                camera::main::run(channels.clone(), camera_cmd_receiver)
            });
        }

        if let Some(image_config) = config.image {
            tasks.add("image download", {
                image::run(channels.clone(), image_config)
            });
        }

        if let Some(gimbal_config) = config.gimbal {
            panic!("gimbal not implemented");
        }

        if let Some(gs_config) = config.ground_server {
            tasks.add("ground server", {
                let gs_client = GroundServerClient::new(channels.clone(), gs_config.address)?;
                async move { gs_client.run().await }
            });
        }

        if let Some(scheduler_config) = config.scheduler {
            tasks.add("scheduler", {
                let mut scheduler = Scheduler::new(channels.clone(), scheduler_config.gps);
                async move { scheduler.run().await }
            });
        }

        if let Some(aux_config) = config.aux_camera {
            #[cfg(feature = "gstreamer")]
            if let Some(stream_config) = aux_config.stream {
                tasks.add("aux camera live stream", {
                    let mut stream_client = camera::aux::stream::StreamClient::connect(
                        channels.clone(),
                        stream_cmd_receiver,
                        stream_config.address,
                        aux_config.cameras.clone(),
                    )?;
                    async move { stream_client.run().await }
                });
            }

            #[cfg(feature = "gstreamer")]
            if let Some(save_config) = aux_config.save {
                tasks.add("aux camera live record", {
                    let mut save_client = camera::aux::save::SaveClient::connect(
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
