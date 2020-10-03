use std::{rc::Rc, sync::atomic::AtomicBool, sync::atomic::Ordering, sync::Arc};

use camera::interface::CameraInterface;
use pixhawk::state::PixhawkMessage;
use tokio::sync::broadcast;

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

struct Channels {
    pixhawk: Option<broadcast::Receiver<PixhawkMessage>>,
    // camera: Option<broadcast::Receiver<CameraMessage>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let loop_continue = Arc::new(AtomicBool::new(true));

    // let pixhawk_task = tokio::spawn({
    //     let loop_continue = loop_continue.clone();

    //     async move {
    //         info!("connecting to pixhawk");

    //         // pixhawk telemetry should be exposed on localhost:5763 for SITL
    //         // TODO: add case for when it's not the SITL
    //         let mut client = pixhawk::client::PixhawkClient::connect(":::5763").await?;

    //         info!("initializing pixhawk");
    //         client.init().await?;

    //         while loop_continue.load(Ordering::Relaxed) {
    //             let msg = client.recv().await?;
    //             trace!("received message: {:?}", msg);
    //         }

    //         Result::<(), anyhow::Error>::Ok(())
    //     }
    // });

    // let _ = pixhawk_task.await?;

    let mut camera = CameraInterface::new();

    info!("connecting to camera");

    camera.connect()?;

    info!("taking photo");

    camera.take_photo()?;

    info!("disconnecting from camera");

    camera.disconnect()?;

    info!("done");

    Ok(())
}
