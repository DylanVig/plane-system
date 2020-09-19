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

    Ok(())
}
