use std::time::SystemTime;

use crate::state::{Attitude, Point3D};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Telemetry {
    pub coords: Option<Point3D>,

    #[serde(with = "serde_millis")]
    pub coords_timestamp: Option<SystemTime>,

    pub attitude: Option<Attitude>,

    #[serde(with = "serde_millis")]
    pub attitude_timestamp: Option<SystemTime>,
}

#[derive(Debug, Clone)]
pub enum PixhawkEvent {
    Image {
        time: SystemTime,
        foc_len: f32,
        img_idx: u16,
        cam_idx: u8,
        flags: mavlink::ardupilotmega::CameraFeedbackFlags,
        coords: Point3D,
        attitude: Attitude,
    },
    Gps {
        position: Point3D,
        /// Velocity in meters per second (X, Y, Z) / (East, North, Up)
        velocity: (f32, f32, f32),
    },
    Orientation {
        attitude: Attitude,
    },
}

// TODO
pub type PixhawkCommand = ();
