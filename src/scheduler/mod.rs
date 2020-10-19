use std::sync::Arc;
use crate::Channels;

mod backend;
mod state;

use crate::scheduler::backend::*;

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
        let mut telemetry_recv = self.channels.telemetry.subscribe();
        let mut interrupt_recv = self.channels.interrupt.subscribe();
        let mut counter = 0;
        loop {
            let telemetry = Channels::realtime_recv(&mut telemetry_recv).await;
            self.backend.update_telemetry(telemetry);
            if let Some(capture_request) = self.backend.get_capture_request() {
                debug!("Got a capture request: {:?}", capture_request);
            }
            if counter == 100 {
                self.backend.set_capture_response();
                counter = 0;
            } else {
                counter += 1;
            }
            if let Ok(_) = interrupt_recv.try_recv() { break; }
        }
        Ok(())
    }
}