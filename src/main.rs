#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate num_derive;
#[macro_use]
extern crate async_trait;

mod client;
mod controller;

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();

    smol::block_on(server::serve())?;

    Ok(())
}
