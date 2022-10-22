use std::{net::SocketAddr, path::{PathBuf, Path}};

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
    pub download: Option<ps_download::DownloadConfig>,
    pub main_camera: Option<ps_main_camera::MainCameraConfig>,

    #[cfg(feature = "aux-camera")]
    pub aux_camera: Option<ps_aux_camera::AuxCameraConfig>,
}

impl PlaneSystemConfig {
    pub fn read_from_path(path: &'_ Path) -> Result<Self, ConfigError> {
        Config::builder()
            .add_source(File::from(path))
            .build()?
            .try_deserialize()
    }
}
