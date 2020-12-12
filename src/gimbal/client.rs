use anyhow::Context;
use num_traits::FromPrimitive;
use simplebgc::*;
use std::sync::Arc;
use std::time::Duration;

use tokio::{sync::mpsc, time::sleep};

use crate::Channels;

use super::{
    interface::{GimbalInterface, GimbalKind, HardwareGimbalInterface, SoftwareGimbalInterface},
    GimbalCommand, GimbalRequest, GimbalResponse,
};

pub struct GimbalClient {
    iface: Box<dyn GimbalInterface + Send>,
    channels: Arc<Channels>,
    cmd: mpsc::Receiver<GimbalCommand>,
}

impl GimbalClient {
    /// Connects to a physical hardware gimbal.
    pub fn connect(
        channels: Arc<Channels>,
        cmd: mpsc::Receiver<GimbalCommand>,
        kind: GimbalKind,
    ) -> anyhow::Result<Self> {
        let iface: Box<dyn GimbalInterface + Send> = match kind {
            GimbalKind::Hardware => Box::new(
                HardwareGimbalInterface::new()
                    .context("failed to create hardware gimbal interface")?,
            ),
            GimbalKind::Software => Box::new(
                SoftwareGimbalInterface::new()
                    .context("failed to create software gimbal interface")?,
            ),
        };

        Ok(Self {
            iface,
            channels,
            cmd,
        })
    }

    pub fn init(&self) -> anyhow::Result<()> {
        trace!("initializing gimbal");
        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.init()?;

        let mut interrupt_recv = self.channels.interrupt.subscribe();

        loop {
            if let Ok(cmd) = self.cmd.try_recv() {
                let result = self.exec(cmd.request()).await;
                let _ = cmd.respond(result);
            }

            self.iface.recv_command();

            if interrupt_recv.try_recv().is_ok() {
                break;
            }

            sleep(Duration::from_millis(10)).await;
        }

        Ok(())
    }

    async fn exec(&mut self, cmd: &GimbalRequest) -> anyhow::Result<GimbalResponse> {
        match cmd {
            GimbalRequest::Control { roll, pitch } => {
                let mut roll = *roll;
                let mut pitch = *pitch;

                info!("got request for {}, {}", roll, pitch);

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

                self.iface.send_command(command)?;
                // TODO: we need to implement CMD_CONFIRM in the simplebgc-rs crate
                // let response = self.get_response()?;
            }
        }

        Ok(GimbalResponse::Unit)
    }
}
