use std::{rc::Rc, sync::atomic::AtomicBool, sync::atomic::Ordering, sync::Arc};

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
mod roi_download;

mod state;

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    let loop_continue = Arc::new(AtomicBool::new(true));

    let pixhawk_task = smol::spawn({
        let loop_continue = loop_continue.clone();

        async move {
            info!("connecting to pixhawk");
            let mut client = pixhawk::client::PixhawkClient::connect(":::14551").await?;

            info!("initializing pixhawk");
            client.init().await?;

            while loop_continue.load(Ordering::Relaxed) {
                let msg = client.recv().await?;
                trace!("received message: {:?}", msg);
            }

            Result::<(), anyhow::Error>::Ok(())
        }
    });

    smol::block_on(pixhawk_task)?;

    Ok(())
}
