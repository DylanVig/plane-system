use std::net::SocketAddr;

use mavlink::MavlinkVersion;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PixhawkConfig {
    pub address: SocketAddr,
    pub mavlink: String,
}
