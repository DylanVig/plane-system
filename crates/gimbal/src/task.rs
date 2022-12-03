use std::fmt::LowerHex;

use anyhow::Context;
use log::{debug, info, warn};

use async_trait::async_trait;
use ps_client::{ChannelCommandSource, Task, ChannelCommandSink};
use simplebgc::{
    AxisControlFlags, AxisControlMode, AxisControlParams, AxisControlState, ControlData,
    ControlFormat, Message, OutgoingCommand, RollPitchYaw,
};
use tokio::select;
use tokio_util::sync::CancellationToken;
use enumflags2::BitFlag;

use crate::{GimbalConfig, GimbalRequest, GimbalResponse};

pub struct GimbalTask {
    cmd_tx: ChannelCommandSink<GimbalRequest, GimbalResponse>,
    cmd_rx: ChannelCommandSource<GimbalRequest, GimbalResponse>,
}

pub fn create_task(config: GimbalConfig) -> anyhow::Result<GimbalTask> {
    let (cmd_tx, cmd_rx) = flume::bounded(256);
    Ok((GimbalTask { cmd_tx, cmd_rx }))
}

impl GimbalTask {
    pub fn cmd(&self) -> ChannelCommandSink<GimbalRequest, GimbalResponse> {
        self.cmd_tx.clone()
    }
}

#[async_trait]
impl Task for GimbalTask {
    fn name(&self) -> &'static str {
        "gimbal"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self { cmd_rx, .. } = *self;

        let loop_fut = async {
            while let Ok((cmd, ret_tx)) = cmd_rx.recv_async().await {
                let result = 'cmd: {
                    match cmd {
                        GimbalRequest::Debug { roll, pitch } => {
                            let factor: f32 = (1 << 14) as f32 / 360.0;

                            let cmd = OutgoingCommand::Control(ControlData {
                                mode: ControlFormat::Extended(RollPitchYaw {
                                    roll: AxisControlState {
                                        mode: AxisControlMode::Angle,
                                        flags: AxisControlFlags::empty(),
                                    },
                                    pitch: AxisControlState {
                                        mode: AxisControlMode::Angle,
                                        flags: AxisControlFlags::empty(),
                                    },
                                    yaw: AxisControlState {
                                        mode: AxisControlMode::NoControl,
                                        flags: AxisControlFlags::empty(),
                                    },
                                }),
                                axes: RollPitchYaw {
                                    roll: AxisControlParams { angle: (roll * factor) as i16, speed: 0 },
                                    pitch: AxisControlParams { angle: (pitch * factor) as i16, speed: 0 },
                                    yaw: AxisControlParams { angle: 0, speed: 0 },
                                },
                            });

                            info!("angle: {roll} {pitch}");

                            let cmd_bytes = cmd.to_v2_bytes();
                            let mut cmd_bytes_hex = String::new();

                            for b in cmd_bytes {
                                use std::fmt::Write;
                                write!(cmd_bytes_hex, "{:02x}", b);
                            }

                            info!("{}", cmd.command_id());
                            info!("{} {}", &cmd_bytes_hex[..], &cmd_bytes_hex[8..cmd_bytes_hex.len() - 4]);


                            Ok(())
                        }
                    }
                };

                ret_tx.send(result).unwrap();
            }

            Ok::<_, anyhow::Error>(())
        };

        select! {
          _ = cancel.cancelled() => {}
          res = loop_fut => { res? }
        }

        Ok(())
    }
}
