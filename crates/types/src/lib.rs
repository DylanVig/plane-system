use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point3D {
    #[serde(serialize_with = "ps_serde_util::serialize_point")]
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
