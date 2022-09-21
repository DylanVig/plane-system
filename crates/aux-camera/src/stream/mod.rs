pub mod task;
pub mod command;
mod interface;

use std::net::SocketAddr;

pub use task::*;
pub use command::*;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct StreamConfig {
    /// Address where footage from auxiliary cameras should be streamed to.
    pub address: SocketAddr,

    /// A list of gstreamer camera specifications
    pub cameras: Vec<String>,
}
