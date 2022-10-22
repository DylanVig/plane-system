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

// TODO
pub type PixhawkCommand = ();
