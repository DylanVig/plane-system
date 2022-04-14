use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    Idle,
    Fixed,
    Tracking,
    OffAxis, // TODO figure out logic for off-axis targets
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point3D {
    pub point: geo::Point<f32>,

    /// Altitude in meters above mean sea level
    pub altitude_msl: f32,

    /// Altitude in meters above the ground
    pub altitude_rel: f32,
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Attitude {
    /// Roll in degrees
    pub roll: f32,

    /// Pitch in degrees
    pub pitch: f32,

    /// Yaw in degrees
    pub yaw: f32,
}

impl Attitude {
    pub fn new(roll: f32, pitch: f32, yaw: f32) -> Self {
        Attitude { roll, pitch, yaw }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Telemetry {
    pub plane_attitude: Attitude,
    pub gimbal_attitude: Attitude,
    pub position: Point3D,
    pub time: chrono::DateTime<chrono::Local>,
}

impl Default for Telemetry {
    fn default() -> Self {
        Telemetry {
            gimbal_attitude: Default::default(),
            plane_attitude: Default::default(),
            position: Default::default(),
            time: chrono::Local::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    path: PathBuf,
    mode: Mode,
    geotag: geo::Point<f32>,
}
