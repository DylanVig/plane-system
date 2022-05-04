use num_traits::FromPrimitive;
use serde::Deserialize;

// real gimbal
pub mod hardware;

// virtual gimbal
pub mod software;

pub use hardware::*;
pub use software::*;

use simplebgc::*;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Deserialize)]
pub enum GimbalProtocol {
    SimpleBGC,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum GimbalKind {
    Hardware { protocol: GimbalProtocol },
    Software,
}

#[async_trait]
pub trait GimbalInterface: Send {
    async fn control_angles(&mut self, roll: f64, pitch: f64) -> anyhow::Result<()>;
}

#[async_trait]
pub trait SimpleBgcGimbalInterface: GimbalInterface {
    async fn send_command(&mut self, cmd: OutgoingCommand) -> anyhow::Result<()>;

    async fn recv_command(&mut self) -> anyhow::Result<Option<IncomingCommand>>;
}

#[async_trait]
impl<T: SimpleBgcGimbalInterface> GimbalInterface for T {
    async fn control_angles(&mut self, roll: f64, pitch: f64) -> anyhow::Result<()> {
        let factor: f64 = 0.02197265625;

        let command = OutgoingCommand::Control(ControlData {
            mode: ControlFormat::Legacy(AxisControlState::from_u8(0x02).unwrap()),
            axes: RollPitchYaw {
                roll: AxisControlParams {
                    /// unit conversion: SBGC units are 360 / 2^14 degrees
                    angle: (roll / factor) as i16,
                    speed: 0,
                },
                pitch: AxisControlParams {
                    /// unit conversion: SBGC units are 360 / 2^14 degrees
                    angle: (pitch / factor) as i16,
                    speed: 0,
                },
                yaw: AxisControlParams { angle: 0, speed: 0 },
            },
        });

        self.send_command(command).await?;

        Ok(())
    }
}
