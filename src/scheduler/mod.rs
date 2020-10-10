use std::{
    sync::Arc,
    time::Duration,
};
use tokio::time::timeout;
use crate::{
    state::RegionOfInterest,
    Channels,
    pixhawk::state::PixhawkMessage,
};

/// Controls whether the plane is taking pictures of the ground (first-pass),
/// taking pictures of ROIs (second-pass), or doing nothing. Coordinates sending
/// requests to the camera and to the gimbal based on telemetry information
/// received from the Pixhawk.
pub struct Scheduler {
    /// List of regions of interest that should be photographed as soon as
    /// possible. Scheduler will prioritize attempting to photograph nearby ROIs
    /// over increasing ground coverage.
    rois: Vec<RegionOfInterest>,

    /// Channel for receiving from the pixhawk client
    channels: Arc<Channels>,
}

impl Scheduler {
    pub fn new(channels: Arc<Channels>) -> Self {
        Self::with_rois(Vec::new(), channels)
    }

    pub fn with_rois(rois: Vec<RegionOfInterest>, channels: Arc<Channels>) -> Self {
        Self {
            rois,
            channels,
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let mut pixhawk_recv = self.channels.pixhawk.subscribe();
        let mut interrupt_recv = self.channels.interrupt.subscribe();
        loop {
            if let Ok(message) = timeout(Duration::from_millis(10), pixhawk_recv.recv()).await {
                match message? {
                    PixhawkMessage::Image {
                        time,
                        foc_len,
                        img_idx,
                        cam_idx,
                        flags,
                        coords,
                        attitude,
                    } => (),
                    PixhawkMessage::Gps { coords: Coords3D } => (),
                    PixhawkMessage::Orientation { attitude: Attitude } => (),
                }
            }


            if let Ok(_) = timeout(Duration::from_millis(10), interrupt_recv.recv()).await { break; }
        }
        Ok(())
    }
}

pub enum SchedulerEvent {
    ROI(RegionOfInterest),
    Coverage,
}
