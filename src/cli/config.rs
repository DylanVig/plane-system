use std::{net::SocketAddr, path::PathBuf};

use config::ConfigError;
use ps_pixhawk::PixhawkConfig;
use serde::Deserialize;


#[derive(Debug, Deserialize)]
pub struct PlaneServerConfig {
    pub address: SocketAddr,
}

#[derive(Debug, Deserialize)]
pub struct PlaneSystemConfig {
    pub pixhawk: Option<PixhawkConfig>,
    // pub plane_server: PlaneServerConfig,
    // pub ground_server: Option<GsConfig>,
    // pub download: Option<DownloadConfig>,
    // pub main_camera: Option<MainCameraConfig>,
    // pub aux_camera: Option<AuxCameraConfig>,
}

impl PlaneSystemConfig {
    pub fn read() -> Result<Self, ConfigError> {
        use config::*;

        let mut c = Config::new();

        c.merge(File::with_name("config/plane-system.json").format(FileFormat::Json))?;
        // c.merge(File::with_name("plane-system.toml").format(FileFormat::Toml))?;
        c.merge(Environment::with_prefix("PLANE_SYSTEM"))?;

        c.try_into()
    }

    pub fn read_from_path(path: PathBuf) -> Result<Self, ConfigError> {
        use config::*;

        let mut c = Config::new();

        c.merge(File::from(path))?;
        c.merge(Environment::with_prefix("PLANE_SYSTEM"))?;

        c.try_into()
    }
}
