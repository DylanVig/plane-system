#[macro_use] extern crate log;
#[macro_use] extern crate anyhow;

mod airdrop;
mod camera;
mod client;
mod scheduler;
mod server;

fn main() -> anyhow::Result<()> {
    pretty_env_logger::init();
    
    smol::block_on(server::serve())?;

    Ok(())
}
