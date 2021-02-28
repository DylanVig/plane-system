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
pub enum GimbalKind {
    Hardware { protocol: GimbalProtocol },
    Software
}

#[async_trait]
pub trait GimbalInterface: Send {
    fn new() -> anyhow::Result<Self>
    where
        Self: Sized;

    async fn send_command(&mut self, cmd: OutgoingCommand) -> anyhow::Result<()>;

    async fn recv_command(&mut self) -> anyhow::Result<Option<IncomingCommand>>;

    async fn control_angles(&mut self, mut roll: f64, mut pitch: f64) -> anyhow::Result<()> {
        info!("Got request for {}, {}", roll, pitch);
        if roll.abs() > 50.0 || pitch.abs() > 50.0 {
            roll = 0.0;
            pitch = 0.0;
        }

        let factor: f64 = (2 ^ 14) as f64 / 360.0;

        let command = OutgoingCommand::Control(ControlData {
            mode: ControlFormat::Legacy(AxisControlState::from_u8(0x02).unwrap()),
            axes: RollPitchYaw {
                roll: AxisControlParams {
                    /// unit conversion: SBGC units are 360 / 2^14 degrees
                    angle: (roll * factor) as i16,
                    speed: 1200,
                },
                pitch: AxisControlParams {
                    /// unit conversion: SBGC units are 360 / 2^14 degrees
                    angle: (pitch * factor) as i16,
                    speed: 2400,
                },
                yaw: AxisControlParams { angle: 0, speed: 0 },
            },
        });

        self.send_command(command).await?;

        // TODO: we need to implement CMD_CONFIRM in the simplebgc-rs crate
        // let response = self.get_response()?;

        Ok(())
    }
}

