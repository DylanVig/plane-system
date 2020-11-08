use std::{sync::Arc, time::Duration};

use anyhow::Context;
use camera::interface::SonyDevicePropertyCode;
use ctrlc;
use pixhawk::{client::PixhawkClient, state::PixhawkMessage};
use ptp::PtpData;
use scheduler::Scheduler;
use state::Telemetry;
use structopt::StructOpt;
use telemetry::TelemetryStream;
use tokio::{spawn, sync::broadcast, task::spawn_blocking};

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
    cli: broadcast::Sender<PixhawkMessage>,
    // camera: Option<broadcast::Receiver<CameraMessage>>,
}

impl Channels {
    pub fn new() -> Self {
        let (interrupt_sender, _) = broadcast::channel(1);
        let (telemetry_sender, _) = broadcast::channel(1);
        let (pixhawk_sender, _) = broadcast::channel(64);
        let (cli_sender, _) = broadcast::channel(1024);

        Channels {
            interrupt: interrupt_sender,
            pixhawk: pixhawk_sender,
            telemetry: telemetry_sender,
            cli: cli_sender,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    // TEST CODE
    {
        debug!("initializing camera");
        let mut camera = camera::interface::CameraInterface::new()?;
        debug!("opening connection to camera");
        camera.connect()?;
        debug!("getting current camera properties");
        camera.update()?;
        debug!("got camera properties");
        camera.set(SonyDevicePropertyCode::OperatingMode, PtpData::UINT8(0x04))?;
        debug!("setting operating mode to content transfer");

        tokio::time::delay_for(Duration::from_secs(3)).await;

        camera.update()?;

        let operating_mode = camera
            .get(SonyDevicePropertyCode::OperatingMode)
            .unwrap()
            .current;

        if operating_mode != PtpData::UINT8(0x04) {
            error!(
                "setting operating mode did not work, current mode is {:?}",
                operating_mode
            );
        }

        let storage_ids = camera.storage_ids()?;

        for storage_id in storage_ids {
            trace!("found storage 0x{:08x}", storage_id);

            let object_ids = camera.object_handles(storage_id)?;
            for object_id in object_ids {
                trace!("\tfound object 0x{:08x}", object_id);
                let object_info = camera.object_info(object_id);
                trace!("\t\tobject info: {:?}", object_info);
            }
        }

        camera.disconnect()?;

        return Ok(());
    }

    let main_args: cli::args::MainArgs = cli::args::MainArgs::from_args();
    let config = if let Some(config_path) = main_args.config {
        debug!("reading config from {:?}", &config_path);
        cli::config::PlaneSystemConfig::read_from_path(config_path)
    } else {
        debug!("reading config from default location");
        cli::config::PlaneSystemConfig::read()
    };

    let config = config.context("failed to read config file")?;

    let channels: Arc<Channels> = Arc::new(Channels::new());

    ctrlc::set_handler({
        let channels = channels.clone();

        move || {
            info!("received interrupt, shutting down");
            let _ = channels.interrupt.send(());
        }
    })
    .expect("could not set ctrl+c handler");

    info!("connecting to pixhawk at {}", &config.pixhawk.address);

    // pixhawk telemetry should be exposed on localhost:5763 for SITL
    // TODO: add case for when it's not the SITL

    let mut pixhawk_client =
        PixhawkClient::connect(channels.clone(), config.pixhawk.address).await?;

    info!("initializing scheduler");
    let scheduler = Scheduler::new(channels.clone());

    info!("initializing telemetry stream");
    let mut telemetry = TelemetryStream::new(channels.clone());

    info!("initializing server");
    let server_address = config
        .server
        .address
        .parse()
        .context("invalid server address")?;

    let pixhawk_task = spawn(async move { pixhawk_client.run().await });
    let scheduler_task = spawn(async move { scheduler.run().await });
    let telemetry_task = spawn(async move { telemetry.run().await });
    let server_task = spawn({
        let channels = channels.clone();
        server::serve(channels, server_address)
    });
    let cli_task = spawn_blocking({
        let channels = channels.clone();
        move || cli::repl::run(channels)
    });

    // wait for any of these tasks to end
    let futures = vec![
        pixhawk_task,
        scheduler_task,
        telemetry_task,
        server_task,
        cli_task,
    ];
    let (result, _, _) = futures::future::select_all(futures).await;
    result?
}
