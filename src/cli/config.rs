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

#[derive(Clone, Debug, Deserialize)]
pub struct ImageConfig {
    /// The folder in which to save downloaded images
    #[serde(default = "default_save_path")]
    pub save_path: PathBuf,
}

fn default_save_path() -> PathBuf {
    std::env::current_dir().expect("could not get current directory")
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Deserialize)]
pub enum CameraKind {
    R10C,
}

#[derive(Debug, Deserialize)]
pub struct MainCameraConfig {
    pub kind: CameraKind,
}

#[derive(Debug, Deserialize)]
pub struct AuxCameraConfig {
    pub stream: Option<AuxCameraStreamConfig>,
    pub save: Option<AuxCameraSaveConfig>,

    // a list of gstreamer camera specifications
    pub cameras: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct AuxCameraStreamConfig {
    pub address: String,
}

#[derive(Debug, Deserialize)]
pub struct AuxCameraSaveConfig {
    pub save_path: String,
}

#[derive(Debug, Deserialize)]
pub struct PlaneSystemConfig {
    pub pixhawk: Option<PixhawkConfig>,
    pub plane_server: PlaneServerConfig,
    pub ground_server: Option<GroundServerConfig>,
    pub image: Option<ImageConfig>,
    pub main_camera: Option<MainCameraConfig>,
    pub aux_camera: Option<AuxCameraConfig>,
    pub gimbal: Option<GimbalConfig>,
    pub scheduler: Option<SchedulerConfig>,
    #[serde(default = "bool::default")]
    pub dummy: bool,
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
