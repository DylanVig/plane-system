use async_trait::async_trait;
use enumflags2::{BitFlag, _internal::RawBitFlags};
use num_traits::FromPrimitive;
use serde::Deserialize;

// real gimbal
pub mod hardware;

// virtual gimbal
pub mod software;

pub use hardware::*;
pub use software::*;

use simplebgc::*;
use tracing::log::debug;

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
        let factor: f64 = (1 << 14) as f64 / 360.0;

        let command = OutgoingCommand::Control(ControlData {
            mode: ControlFormat::Extended(RollPitchYaw {
                roll: AxisControlState {
                    mode: AxisControlMode::Angle,
                    flags: AxisControlFlags::AutoTask.into(),
                },
                pitch: AxisControlState {
                    mode: AxisControlMode::Angle,
                    flags: AxisControlFlags::AutoTask.into(),
                },
                yaw: AxisControlState {
                    mode: AxisControlMode::NoControl,
                    flags: AxisControlFlags::empty(),
                },
            }),
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

        debug!(
            "Sending command: {} {:x} {:x}",
            command.command_id(),
            command.to_payload_bytes(),
            command.to_v2_bytes()
        );
        self.send_command(command).await?;

        debug!("Receiving command");
        let response = self.recv_command().await?;

        if let Some(response) = response {
            debug!(
                "Received command: {} {:x} {:x}",
                response.command_id(),
                response.to_payload_bytes(),
                response.to_v2_bytes()
            );
        } else {
            debug!("Received no command")
        }

        Ok(())
    }
}
