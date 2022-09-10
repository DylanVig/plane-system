pub mod task;
pub mod command;
mod interface;

use std::net::SocketAddr;

pub use task::*;
pub use command::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct StreamConfig {
    pub address: SocketAddr,
}
