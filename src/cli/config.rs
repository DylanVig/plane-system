use std::{net::SocketAddr, path::PathBuf};

use config::ConfigError;
use mavlink::MavlinkVersion;
use serde::Deserialize;

use crate::{gimbal::GimbalKind, state::Coords2D};

#[derive(Debug, Deserialize)]
pub struct PixhawkConfig {
    pub address: SocketAddr,
    pub mavlink: MavlinkVersion,
}

#[derive(Debug, Deserialize)]
pub struct PlaneServerConfig {
    pub address: SocketAddr,
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
pub struct CurrentSensingConfig {
    pub gpio_int: u8,
    pub gpio_ack: u8,
    pub i2c: u8,
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
    pub address: SocketAddr,
}

#[derive(Debug, Deserialize)]
pub struct AuxCameraSaveConfig {
    pub save_path: PathBuf,
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
}

impl PlaneSystemConfig {
    pub fn read() -> Result<Self, ConfigError> {
        use config::*;

        let mut c = Config::new();

        c.merge(File::with_name("plane-system.json").format(FileFormat::Json))?;
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
