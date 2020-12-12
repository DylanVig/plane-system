// real gimbal
pub mod hardware;

// virtual gimbal
pub mod software;

pub use hardware::*;
pub use software::*;

use serde::{Deserialize, Serialize};

pub trait GimbalInterface {
    fn new() -> anyhow::Result<Self>
    where
        Self: Sized;

    fn control_angles(&mut self, roll: f64, pitch: f64) -> anyhow::Result<()>;
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
pub enum GimbalKind {
    Hardware,
    Software,
}
