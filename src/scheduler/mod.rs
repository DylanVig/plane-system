use anyhow::Context;

use crate::{gimbal::GimbalRequest, state::Coords2D, Channels, Command};

use std::sync::Arc;

mod backend;
mod state;

use backend::*;

/// Controls whether the plane is taking pictures of the ground (first-pass),
/// taking pictures of ROIs (second-pass), or doing nothing. Coordinates sending
/// requests to the camera and to the gimbal based on telemetry information
/// received from the Pixhawk.
pub struct Scheduler {
    /// Channel for receiving from the pixhawk client
    channels: Arc<Channels>,
    backend: SchedulerBackend,
}

impl Scheduler {
    pub fn new(channels: Arc<Channels>, gps: Coords2D) -> Self {
        Self {
            channels,
            backend: SchedulerBackend::new(gps),
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        // telemetry_recv can hang indefinitely if there is no pixhawk, so we
        // need to do a select() to avoid this

        let mut interrupt_recv = self.channels.interrupt.subscribe();
        let interrupt_fut = interrupt_recv.recv();

        let mut telemetry_recv = self.channels.telemetry.clone();
        let loop_fut = async move {
            loop {
                telemetry_recv
                    .changed()
                    .await
                    .context("telemetry channel closed")?;

                if let Some(telemetry) = telemetry_recv.borrow().as_ref() {
                    self.backend.update_telemetry(telemetry.clone());
                }

                if let Some(capture_request) = self.backend.get_capture_request() {
                    debug!("Got a capture request: {:?}", capture_request);
                }

                let (roll, pitch) = self.backend.get_target_gimbal_angles();
                let request = GimbalRequest::Control { roll, pitch };
                let (cmd, _) = Command::new(request);
                self.channels.gimbal_cmd.clone().send(cmd)?;
            }

            // this is necessary so that Rust can figure out what the return
            // type of the async block is
            #[allow(unreachable_code)]
            Result::<(), anyhow::Error>::Ok(())
        };

        futures::pin_mut!(loop_fut);
        futures::pin_mut!(interrupt_fut);
        futures::future::select(interrupt_fut, loop_fut).await;

        Ok(())
    }
}
