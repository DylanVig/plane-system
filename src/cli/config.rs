use std::path::PathBuf;

use config::{Config, ConfigError};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PixhawkConfig {
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct PlaneSystemConfig {
    pub pixhawk: PixhawkConfig,
}

impl PlaneSystemConfig {
    pub fn read() -> Result<Self, ConfigError> {
        let mut c = Config::new();

        c.merge(config::File::with_name("plane-system"))?;
        c.merge(config::Environment::with_prefix("PLANE_SYSTEM"))?;

        c.try_into()
    }

    pub fn read_from_path(path: PathBuf) -> Result<Self, ConfigError> {
        let mut c = Config::new();

        c.merge(config::File::from(path))?;
        c.merge(config::Environment::with_prefix("PLANE_SYSTEM"))?;

        c.try_into()
    }
}
