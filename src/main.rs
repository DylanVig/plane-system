use std::{sync::atomic::AtomicBool, sync::atomic::Ordering, sync::Arc};

use ctrlc;
use pixhawk::{client::PixhawkClient, state::PixhawkMessage};
use tokio::{spawn, sync::broadcast};

#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate num_derive;

mod camera;
mod gimbal;
mod gpio;
mod image_upload;
mod pixhawk;
mod server;

mod state;

#[derive(Debug)]
pub struct Channels {
    /// Channel for broadcasting a signal when the server should terminate.
    interrupt: broadcast::Sender<()>,

    /// Channel for broadcasting updates to the state of the Pixhawk.
    pixhawk: broadcast::Sender<PixhawkMessage>,
    // camera: Option<broadcast::Receiver<CameraMessage>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let (interrupt_sender, _) = broadcast::channel(1);
    let (pixhawk_sender, _) = broadcast::channel(1024);

    let channels: Arc<Channels> = Arc::new(Channels {
        interrupt: interrupt_sender,
        pixhawk: pixhawk_sender,
    });

    ctrlc::set_handler({
        let channels = channels.clone();

        move || {
            let _ = channels.interrupt.send(());
        }
    })
    .expect("could not set ctrl+c handler");

    info!("connecting to pixhawk");

    // pixhawk telemetry should be exposed on localhost:5763 for SITL
    // TODO: add case for when it's not the SITL

    let pixhawk_client = PixhawkClient::connect(channels.clone(), ":::5763").await?;


    let pixhawk_task = spawn(async move { pixhawk_client.run().await });
    let server_task = spawn(async { server::serve().await });

    let (pixhawk_result, server_result) = futures::future::join(pixhawk_task, server_task).await;

    pixhawk_result??;
    server_result??;

    Ok(())
}
