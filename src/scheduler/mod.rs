use anyhow::Context;

use crate::{
    gimbal::GimbalRequest, 
    gimbal::GimbalResponse,
    Channels, 
    Command,
};

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
    pub fn new(channels: Arc<Channels>) -> Self {
        Self {
            channels,
            backend: SchedulerBackend::new(),
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut telemetry_recv = self.channels.telemetry.clone();
        let mut interrupt_recv = self.channels.interrupt.clone();

        loop {
            let telemetry = telemetry_recv
                .recv()
                .await
                .context("telemetry channel closed")?;

            if let Some(telemetry) = telemetry {
                self.backend.update_telemetry(telemetry);
            }

            if let Some(capture_request) = self.backend.get_capture_request() {
                debug!("Got a capture request: {:?}", capture_request);
            }

            let (roll, pitch) = self.backend.get_target_gimbal_angles();
            let request = GimbalRequest::Control {
                roll,
                pitch,
            };
            let (cmd, _) = Command::new(request);
            self.channels.gimbal_cmd.clone().send(cmd).await?;

            if *interrupt_recv.borrow() {
                break;
            }
        }

        Ok(())
    }
}
