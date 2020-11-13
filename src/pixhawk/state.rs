use std::time::SystemTime;

use crate::state::{Attitude, Coords3D};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Telemetry {
    pub coords: Option<Coords3D>,

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
        coords: Coords3D,
        attitude: Attitude,
    },
    Gps {
        coords: Coords3D,
    },
    Orientation {
        attitude: Attitude,
    },
}

// TODO
pub type PixhawkCommand = ();
