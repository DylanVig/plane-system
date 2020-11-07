use std::sync::Arc;

use state::Telemetry;
use ctrlc;
use pixhawk::{client::PixhawkClient, state::PixhawkMessage};
use scheduler::Scheduler;
use telemetry::TelemetryStream;
use anyhow::Context;
use structopt::StructOpt;
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
mod gimbal;
mod gpio;
mod image_upload;
mod pixhawk;
mod scheduler;
mod telemetry;
mod server;
mod util;
mod cli;
mod state;

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

    // TEST CODE
    {
        debug!("initializing camera");
        let mut camera = camera::interface::CameraInterface2::new()?;
        debug!("opening connection to camera");
        camera.connect()?;

        return Ok(());
    }

    // wait for any of these tasks to end
    let futures = vec![pixhawk_task, scheduler_task, telemetry_task, server_task, cli_task];
    let (result, _, _) = futures::future::select_all(futures).await;
    result?
}
