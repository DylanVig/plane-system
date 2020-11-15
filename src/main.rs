use std::sync::Arc;

use anyhow::Context;
use camera::{client::CameraClient, command::CameraRequest, state::CameraEvent};
use ctrlc;
use gimbal::client::GimbalClient;
use pixhawk::{client::PixhawkClient, state::PixhawkEvent};
use scheduler::Scheduler;
use state::TelemetryInfo;
use std::time::Duration;
use structopt::StructOpt;
use telemetry::TelemetryStream;
use tokio::{spawn, sync::*, time::delay_for};

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
    pixhawk_cmd: mpsc::Sender<pixhawk::PixhawkCommand>,

    /// Channel for broadcasting updates to the state of the camera.
    camera_event: broadcast::Sender<CameraEvent>,

    /// Channel for sending instructions to the camera.
    camera_cmd: mpsc::Sender<camera::CameraCommand>,

    /// Channel for sending instructions to the gimbal.
    gimbal_cmd: mpsc::Sender<gimbal::GimbalCommand>,
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
        self.channel().send(result)
    }

    fn success(self, data: Res) -> Result<(), Result<Res, Err>> {
        self.channel().send(Ok(data))
    }

    fn error(self, error: Err) -> Result<(), Result<Res, Err>> {
        self.channel().send(Err(error))
    }
}

#[tokio::main]
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

    let (interrupt_sender, _) = broadcast::channel(1);
    let (telemetry_sender, telemetry_receiver) = watch::channel(None);
    let (pixhawk_event_sender, _) = broadcast::channel(64);
    let (pixhawk_cmd_sender, pixhawk_cmd_receiver) = mpsc::channel(64);
    let (camera_event_sender, _) = broadcast::channel(256);
    let (camera_cmd_sender, camera_cmd_receiver) = mpsc::channel(256);
    let (gimbal_cmd_sender, gimbal_cmd_receiver) = mpsc::channel(256);

    let channels = Arc::new(Channels {
        interrupt: interrupt_sender.clone(),
        telemetry: telemetry_receiver,
        pixhawk_event: pixhawk_event_sender,
        pixhawk_cmd: pixhawk_cmd_sender,
        camera_event: camera_event_sender,
        camera_cmd: camera_cmd_sender,
        gimbal_cmd: gimbal_cmd_sender,
    });

    let mut task_names = Vec::new();
    let mut futures = Vec::new();

    ctrlc::set_handler({
        let interrupt_sender = interrupt_sender.clone();
        move || {
            info!("received interrupt, shutting down");
            let _ = interrupt_sender.send(());
        }
    })
    .expect("could not set ctrl+c handler");

    if let Some(pixhawk_address) = config.pixhawk.address {
        info!("connecting to pixhawk at {}", pixhawk_address);
        let pixhawk_task = spawn({
            let mut pixhawk_client =
                PixhawkClient::connect(channels.clone(), pixhawk_cmd_receiver, pixhawk_address)
                    .await?;
            async move { pixhawk_client.run().await }
        });
        futures.push(pixhawk_task);
        task_names.push("pixhawk");

        info!("initializing telemetry stream");
        let telemetry_task = spawn({
            let telemetry = TelemetryStream::new(channels.clone(), telemetry_sender);
            async move { telemetry.run().await }
        });
        task_names.push("telemetry");
        futures.push(telemetry_task);
    } else {
        info!("pixhawk address not specified, disabling pixhawk connection and telemetry stream");
    }

    info!("initializing continuous capture");
    let continuous_capture_task = spawn({
        let channels = channels.clone();

        async move {
            let mut interrupt_recv = channels.interrupt.subscribe();
            delay_for(Duration::from_millis(10000)).await;
            info!("beginning continuous capture");

            loop {
                let request = CameraRequest::Capture;
                let (cmd, chan) = Command::new(request);

                channels
                    .camera_cmd
                    .clone()
                    .send(cmd)
                    .await
                    .context("could not send command to camera")?;

                let res = chan.await?;

                if let Err(err) = res {
                    error!("continuous capture error: {:?}", err);
                }

                delay_for(Duration::from_millis(1000)).await;

                if interrupt_recv.try_recv().is_ok() {
                    break;
                }
            }

            Ok(())
        }
    });
    task_names.push("continuous capture");
    futures.push(continuous_capture_task);

    if config.camera {
        info!("connecting to camera");
        let camera_task = spawn({
            let mut camera_client = CameraClient::connect(channels.clone(), camera_cmd_receiver)?;
            async move { camera_client.run().await }
        });
        task_names.push("camera");
        futures.push(camera_task);
    }

    if config.gimbal {
        info!("initializing gimbal");
        let gimbal_task = spawn({
            let mut gimbal_client = GimbalClient::connect(channels.clone(), gimbal_cmd_receiver)?;
            async move { gimbal_client.run().await }
        });
        task_names.push("gimbal");
        futures.push(gimbal_task);
    }

    if config.scheduler.enabled {
        info!("initializing scheduler");
        let scheduler_task = spawn({
            let mut scheduler = Scheduler::new(channels.clone(), config.scheduler.gps);
            async move { scheduler.run().await }
        });
        task_names.push("scheduler");
        futures.push(scheduler_task);
    }

    info!("initializing server");
    let server_address = config
        .server
        .address
        .parse()
        .context("invalid server address")?;
    let server_task = spawn({
        let channels = channels.clone();
        server::serve(channels, server_address)
    });
    task_names.push("server");
    futures.push(server_task);

    info!("intializing cli");
    let cli_task = spawn({
        let channels = channels.clone();
        cli::repl::run(channels)
    });
    task_names.push("cli");
    futures.push(cli_task);

    while futures.len() > 0 {
        // wait for each task to end
        let (result, i, remaining) = futures::future::select_all(futures).await;
        let task_name = task_names.remove(i);

        info!(
            "{} ({}) task ended, {} remaining",
            task_name,
            i,
            remaining.len()
        );

        // if a task ended with an error or did not join properly, end the process
        // with an interrupt
        if let Err(err) = result? {
            error!(
                "got error from {} task, sending interrupt: {:?}",
                task_name, err
            );

            info!("remaining tasks: {:?}", task_names.join(", "));

            let _ = interrupt_sender.send(());

            tokio::task::spawn(async {
                tokio::time::delay_for(Duration::from_secs(5)).await;
                warn!("tasks did not end after 5 seconds, force-quitting");
                std::process::exit(1);
            });
        }

        futures = remaining;
    }

    info!("exit");

    Ok(())
}
