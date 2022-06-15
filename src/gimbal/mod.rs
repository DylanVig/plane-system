pub mod client;
pub mod command;
mod interface;

pub use client::*;
pub use command::*;
pub use interface::GimbalKind;

pub struct GimbalPosition {
    /// The roll of the gimbal in degrees.
    roll: f32,
    /// The pitch of the gimbal in degrees.
    pitch: f32,
}
