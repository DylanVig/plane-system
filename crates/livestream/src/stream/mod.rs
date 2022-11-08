pub mod command;
mod interface;
pub mod task;

use std::net::SocketAddr;

pub use command::*;
use serde::Deserialize;
pub use task::*;

#[derive(Clone, Debug, Deserialize)]
pub struct StreamConfig {
    /// Address where footage from auxiliary cameras should be streamed to.
    pub address: SocketAddr,

    /// A list of gstreamer camera specifications
    pub cameras: Vec<String>,
}
