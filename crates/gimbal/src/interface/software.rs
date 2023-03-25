use anyhow::Context;
use simplebgc::*;
use std::time::{Duration, Instant};

use std::f32::consts::PI;
use tokio::{spawn, sync::*, time::interval};

use super::SimpleBgcGimbalInterface;

pub struct SoftwareGimbalInterface {
    tx: broadcast::Sender<OutgoingCommand>,
    rx: broadcast::Receiver<IncomingCommand>,
}

impl SoftwareGimbalInterface {
    pub fn new() -> anyhow::Result<Self> {
        let (tx_out, rx_out) = broadcast::channel(64);
        let (tx_in, rx_in) = broadcast::channel(64);

        let mut state = GimbalState::new(rx_out, tx_in);

        spawn(async move { state.run_forever().await });

        Ok(SoftwareGimbalInterface {
            tx: tx_out,
            rx: rx_in,
        })
    }
}

#[async_trait]
impl SimpleBgcGimbalInterface for SoftwareGimbalInterface {
    async fn send_command(&mut self, cmd: OutgoingCommand) -> anyhow::Result<()> {
        self.tx.send(cmd).context("could not send gimbal command")?;

        Ok(())
    }

    async fn recv_command(&mut self) -> anyhow::Result<Option<IncomingCommand>> {
        Ok(Some(self.rx.recv().await?))
    }
}

/// Our gimbal only has two axes, so for all of these parameters,
/// the yaw will be ignored.
#[derive(Debug)]
pub struct GimbalState {
    // the last time the gimbal state was updated
    last_tick: Option<Instant>,

    // the last time we reported the gimbal state to the user
    last_update: Option<Instant>,

    params: RollPitchYaw<GimbalAxisState>,
    tx: broadcast::Sender<IncomingCommand>,
    rx: broadcast::Receiver<OutgoingCommand>,
}

#[derive(Clone, Debug)]
struct GimbalAxisState {
    angle: f32,
    pid: AxisPidParams,
    control: AxisControlState,
    params: AxisControlParams,
}

/// Logistic function that passes through (0, 0)
fn sigmoid(x: f32) -> f32 {
    let y = f32::exp(x);
    y * 2.0 / (y + 1.0) - 1.0
}

// angle units are 0.02197265625ths of a degree
const ANGLE_SCALE: f32 = 0.02197265625 / 180.0 * PI;

// angular velocity units are 0.1220740379ths of a degree/sec
const ANGULAR_VELOCITY_SCALE: f32 = 0.1220740379 / 180.0 * PI;

impl GimbalState {
    pub fn new(
        rx: broadcast::Receiver<OutgoingCommand>,
        tx: broadcast::Sender<IncomingCommand>,
    ) -> Self {
        let default_params = GimbalAxisState {
            angle: 0.0,
            pid: AxisPidParams {
                p: 0,
                i: 0,
                d: 0,
                invert: false,
                poles: 0,
                power: 0,
            },
            control: AxisControlState {
                mode: AxisControlMode::NoControl,
                flags: Default::default(),
            },
            params: AxisControlParams { speed: 0, angle: 0 },
        };

        GimbalState {
            last_tick: None,
            last_update: None,
            params: RollPitchYaw {
                roll: default_params.clone(),
                pitch: default_params.clone(),
                yaw: default_params.clone(),
            },
            tx,
            rx,
        }
    }

    pub fn tick(&mut self) {
        let delta = self
            .last_tick
            .map_or_else(Duration::default, |last_tick| Instant::now() - last_tick);

        // handle incoming messages
        while let Ok(cmd) = self.rx.try_recv() {
            self.handle(cmd);
        }

        // update params & status
        self.params.update(
            |&mut GimbalAxisState {
                 ref mut angle,
                 control,
                 params,
                 ..
             }| {
                // current_angle is in radians, we need to convert SimpleBGC params to radians also

                let target_angle = params.angle as f32 * ANGLE_SCALE;

                let target_speed = params.speed as f32 * ANGULAR_VELOCITY_SCALE;

                *angle = match control.mode {
                    // PID simulation is too complex for now, but info is stored here anyway
                    // just in case it needs to be implemented later
                    AxisControlMode::NoControl => *angle,
                    AxisControlMode::Speed => *angle + delta.as_secs_f32() * target_speed,
                    AxisControlMode::Angle => {
                        *angle + delta.as_secs_f32() * sigmoid(target_angle - *angle) * target_speed
                    }
                    _ => unimplemented!(),
                };

                *angle %= PI * 2.0;
            },
        );

        if self.last_update.map_or(true, |last_update| {
            Instant::now() - last_update > Duration::from_secs(1)
        }) {
            self.last_update = Some(Instant::now());
            info!(
                "current angle: {:.5}°, {:.5}°, {:.5}°",
                self.params.roll.angle * 180.0 / PI,
                self.params.pitch.angle * 180.0 / PI,
                self.params.yaw.angle * 180.0 / PI
            );
        }

        self.last_tick = Some(Instant::now());
    }

    pub fn handle(&mut self, cmd: OutgoingCommand) {
        match cmd {
            OutgoingCommand::BoardInfo | OutgoingCommand::BoardInfo3 => {
                if let Err(_) = self
                    .tx
                    .send(IncomingCommand::BoardInfo(simplebgc::BoardInfo {
                        board_version: 0,
                        firmware_version: 2800, // 2.80b0
                        state: Default::default(),
                        board_features: Default::default(),
                        connection_flag: ConnectionFlag::USB.into(),
                        frw_extra_id: 0,
                        reserved: [0u8; 7],
                    }))
                {
                    warn!("failed to send message");
                }
            }
            OutgoingCommand::Control(params) => {
                self.params.roll.params = params.axes.roll;
                self.params.pitch.params = params.axes.pitch;
                self.params.yaw.params = params.axes.yaw;

                match params.mode {
                    ControlFormat::Legacy(state) => {
                        self.params.roll.control = state;
                        self.params.pitch.control = state;
                        self.params.yaw.control = state;
                    }
                    ControlFormat::Extended(state) => {
                        self.params.roll.control = state.roll;
                        self.params.pitch.control = state.pitch;
                        self.params.yaw.control = state.yaw;
                    }
                }
            }
            OutgoingCommand::ReadParams(info) => {
                info!("requested info for profile {:?}", info.profile_id);
                warn!("profile info is currently unimplemented");
                // TODO: implement profile info
            }
            OutgoingCommand::GetAngles => {
                if let Err(_) = self
                    .tx
                    .send(IncomingCommand::GetAngles(self.params.as_ref().map(
                        |GimbalAxisState {
                             angle,
                             params: control_params,
                             ..
                         }| AngleInfo {
                            imu_angle: (angle / ANGLE_SCALE) as i16,
                            target_speed: control_params.speed,
                            target_angle: control_params.angle,
                        },
                    )))
                {
                    warn!("failed to send message");
                }
            }
            cmd => {
                warn!("received unimplemented command: {:?}", cmd);
            }
        }
    }

    pub async fn run_while<F: Fn() -> bool>(&mut self, predicate: F) -> () {
        let mut int = interval(Duration::from_millis(16));

        while predicate() {
            self.tick();
            int.tick().await;
        }
    }

    pub async fn run_forever(&mut self) -> () {
        self.run_while(|| true).await
    }
}
