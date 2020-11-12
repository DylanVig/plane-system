use std::{sync::Arc, time::Duration};

use anyhow::Context;
use camera::{client::CameraClient, interface::SonyDevicePropertyCode, state::CameraMessage};
use command::{Command, CommandData, Response, ResponseData};
use ctrlc;
use pixhawk::{client::PixhawkClient, state::PixhawkMessage};
use ptp::PtpData;
use scheduler::Scheduler;
use state::TelemetryInfo;
use structopt::StructOpt;
use telemetry::TelemetryStream;
use tokio::{spawn, sync::{broadcast, mpsc}, sync::{oneshot, watch}, task::spawn_blocking};

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
mod gpio;
mod image_upload;
mod pixhawk;
mod scheduler;
mod server;
mod state;
mod telemetry;
mod util;
mod command;

#[derive(Debug)]
pub struct Channels {
    /// Channel for broadcasting a signal when the server should terminate.
    interrupt: watch::Receiver<bool>,

    /// Channel for broadcasting telemetry information gathered from the gimbal and pixhawk
    telemetry: watch::Receiver<Option<TelemetryInfo>>,

    /// Channel for broadcasting updates to the state of the Pixhawk.
    pixhawk: broadcast::Sender<PixhawkMessage>,

    /// Channel for broadcasting updates to the state of the camera.
    camera: broadcast::Sender<CameraMessage>,
}

#[async_trait]
pub trait Component {
    type Cmd: Command;

    async fn run() -> anyhow::Result<()>;

    async fn exec(command: Self::Cmd) -> anyhow::Result<<Self::Cmd as Command>::Res>;
}

pub trait Command {
    type Res: Response;
    type Data;

    fn channel(&self) -> oneshot::Sender<Self::Res>;
    fn data(&self) -> Self::Data;
}

pub trait Response: serde::Serialize {

}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let main_args: cli::args::MainArgs = cli::args::MainArgs::from_args();

    let config = if let Some(config_path) = main_args.config {
        debug!("reading config from {:?}", &config_path);
        cli::config::PlaneSystemConfig::read_from_path(config_path)
    } else {
        debug!("reading config from default location");
        cli::config::PlaneSystemConfig::read()
    };

    let config = config.context("failed to read config file")?;

    let (interrupt_sender, interrupt_receiver) = watch::channel(false);
    let (telemetry_sender, telemetry_receiver) = watch::channel(None);
    let (pixhawk_sender, _) = broadcast::channel(64);
    let (camera_sender, _) = broadcast::channel(256);

    let channels = Arc::new(Channels {
        interrupt: interrupt_receiver,
        telemetry: telemetry_receiver,
        pixhawk: pixhawk_sender,
        camera: camera_sender,
    });

    let mut futures = Vec::new();

    ctrlc::set_handler({
        let channels = channels.clone();

        move || {
            info!("received interrupt, shutting down");
            let _ = interrupt_sender.broadcast(true);
        }
    })
    .expect("could not set ctrl+c handler");

    if let Some(pixhawk_address) = config.pixhawk.address {
        info!("connecting to pixhawk at {}", pixhawk_address);
        let mut pixhawk_client = PixhawkClient::connect(channels.clone(), pixhawk_address).await?;
        let pixhawk_task = spawn(async move { pixhawk_client.run().await });
        futures.push(pixhawk_task);

        info!("initializing telemetry stream");
        let mut telemetry = TelemetryStream::new(channels.clone());
        let telemetry_task = spawn(async move { telemetry.run().await });
        futures.push(telemetry_task);
    } else {
        info!("pixhawk address not specified, disabling pixhawk connection and telemetry stream");
    }

    if config.camera {
        info!("connecting to camera");
        let camera_task = spawn({
            let mut camera_client = CameraClient::connect(channels.clone())?;
            async move { camera_client.run().await }
        });
        futures.push(camera_task);
    }

    info!("initializing scheduler");
    let scheduler_task = spawn({
        let scheduler = Scheduler::new(channels.clone());
        async move { scheduler.run().await }
    });
    futures.push(scheduler_task);

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
    futures.push(server_task);

    info!("intializing cli");
    let cli_task = spawn({
        let channels = channels.clone();
        cli::repl::run(channels)
    });
    futures.push(cli_task);


    // wait for any of these tasks to end
    let (result, _, _) = futures::future::select_all(futures).await;
    result?
}
