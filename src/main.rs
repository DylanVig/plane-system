#[macro_use] extern crate log;
#[macro_use] extern crate anyhow;
#[macro_use] extern crate num_derive;

mod camera;
mod client;
mod scheduler;
mod server;
mod state;

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    
    smol::block_on(server::serve())?;

    Ok(())
}
