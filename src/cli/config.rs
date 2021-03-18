use std::path::PathBuf;

use config::{Config, ConfigError};
use mavlink::MavlinkVersion;
use serde::Deserialize;

use crate::{gimbal::GimbalKind, state::Coords2D};

#[derive(Debug, Deserialize)]
pub struct PixhawkConfig {
    pub address: String,
    pub mavlink: MavlinkVersion,
}

#[derive(Debug, Deserialize)]
pub struct PlaneServerConfig {
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct GroundServerConfig {
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct SchedulerConfig {
    pub gps: Coords2D,
}

#[derive(Debug, Deserialize)]
pub struct GimbalConfig {
    pub kind: GimbalKind,

    /// The path to the device file
    pub device_path: Option<PathBuf>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Deserialize)]
pub enum CameraKind {
    R10C,
}

#[derive(Debug, Deserialize)]
pub struct CameraConfig {
    pub kind: CameraKind,

    /// The folder in which to save downloaded images
    pub save_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct PlaneSystemConfig {
    pub pixhawk: Option<PixhawkConfig>,
    pub plane_server: PlaneServerConfig,
    pub ground_server: Option<GroundServerConfig>,
    pub camera: Option<CameraConfig>,
    pub gimbal: Option<GimbalConfig>,
    pub scheduler: Option<SchedulerConfig>,
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
