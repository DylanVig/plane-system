use std::{sync::Arc, time::Duration};

use anyhow::Context;
use camera::{client::CameraClient, interface::SonyDevicePropertyCode, state::CameraMessage};
use cli::repl::{CliCommand, CliResult};
use ctrlc;
use pixhawk::{client::PixhawkClient, state::PixhawkMessage};
use ptp::PtpData;
use scheduler::Scheduler;
use state::Telemetry;
use structopt::StructOpt;
use telemetry::TelemetryStream;
use tokio::{
    spawn,
    sync::{broadcast, mpsc},
    task::spawn_blocking,
};

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

#[derive(Debug)]
pub struct Channels {
    /// Channel for broadcasting a signal when the server should terminate.
    interrupt: broadcast::Sender<()>,

    /// Channel for broadcasting updates to the state of the Pixhawk.
    pixhawk: broadcast::Sender<PixhawkMessage>,

    /// Channel for broadcasting telemetry information gathered from the gimbal and pixhawk
    telemetry: broadcast::Sender<Telemetry>,

    /// Channel for broadcasting commands the user enters via the REPL
    cli_cmd: broadcast::Sender<CliCommand>,

    /// Channel for returning results from commands
    cli_result: mpsc::Sender<CliResult>,

    camera: broadcast::Sender<CameraMessage>,
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

    let (interrupt_sender, _) = broadcast::channel(1);
    let (telemetry_sender, _) = broadcast::channel(1);
    let (pixhawk_sender, _) = broadcast::channel(64);
    let (cli_cmd_sender, _) = broadcast::channel(1024);
    let (cli_result_sender, cli_result_receiver) = mpsc::channel(1);
    let (camera_sender, _) = broadcast::channel(256);

    let channels = Arc::new(Channels {
        interrupt: interrupt_sender,
        pixhawk: pixhawk_sender,
        telemetry: telemetry_sender,
        cli_cmd: cli_cmd_sender,
        cli_result: cli_result_sender,
        camera: camera_sender,
    });

    let mut futures = Vec::new();

    ctrlc::set_handler({
        let channels = channels.clone();

        move || {
            info!("received interrupt, shutting down");
            let _ = channels.interrupt.send(());
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
        cli::repl::run(channels, cli_result_receiver)
    });
    futures.push(cli_task);


    // wait for any of these tasks to end
    let (result, _, _) = futures::future::select_all(futures).await;
    result?
}
