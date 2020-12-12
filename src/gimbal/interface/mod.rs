// real gimbal
pub mod hardware;

// virtual gimbal
pub mod software;

pub trait GimbalInterface {
    fn new() -> anyhow::Result<Self> where Self: Sized;

    fn control_angles(&mut self, roll: f64, pitch: f64) -> anyhow::Result<()>;
}
