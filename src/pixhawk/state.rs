use std::time::SystemTime;

use crate::state::{Attitude, Coords3D};
use serde::{Serialize, Deserialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct Telemetry {
    pub coords: Option<Coords3D>,
    pub attitude: Option<Attitude>,
}

#[derive(Debug, Clone)]
pub enum PixhawkMessage {
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
