use std::{net::SocketAddr, path::Path};

use config::{Config, ConfigError, File};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PlaneServerConfig {
    pub address: SocketAddr,
}

#[derive(Debug, Deserialize)]
pub struct PlaneSystemConfig {
    // pub plane_server: PlaneServerConfig,
    pub pixhawk: Option<ps_pixhawk::PixhawkConfig>,
    pub ground_server: Option<ps_gs::GsConfig>,
    pub main_camera: Option<ps_main_camera::MainCameraConfig>,

    #[cfg(feature = "livestream")]
    pub livestream: Option<ps_livestream::LivestreamConfig>,
}

impl PlaneSystemConfig {
    pub fn read_from_path(path: &'_ Path) -> Result<Self, ConfigError> {
        Config::builder()
            .add_source(File::from(path))
            .build()?
            .try_deserialize()
    }
}
