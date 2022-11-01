use serde::{Deserialize, Serialize};
use uom::si::f32::*;

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point3D {
    #[serde(serialize_with = "ps_serde_util::serialize_point")]
    pub point: geo::Point<f32>,

    /// Altitude in meters above mean sea level
    pub altitude_msl: Length,

    /// Altitude in meters above the ground
    pub altitude_rel: Length,
}

#[derive(Default, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Euler {
    pub roll: Angle,
    pub pitch: Angle,
    pub yaw: Angle,
}

impl Euler {
    pub fn new<T: uom::si::angle::Unit + uom::Conversion<f32, T = f32>>(
        roll: f32,
        pitch: f32,
        yaw: f32,
    ) -> Self {
        Self {
            roll: Angle::new::<T>(roll),
            pitch: Angle::new::<T>(pitch),
            yaw: Angle::new::<T>(yaw),
        }
    }
}

#[derive(Default, Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Velocity3D {
    pub x: Velocity,
    pub y: Velocity,
    pub z: Velocity,
}

impl Velocity3D {
    pub fn new<T: uom::si::velocity::Unit + uom::Conversion<f32, T = f32>>(
        x: f32,
        y: f32,
        z: f32,
    ) -> Self {
        Self {
            x: Velocity::new::<T>(x),
            y: Velocity::new::<T>(y),
            z: Velocity::new::<T>(z),
        }
    }
}
