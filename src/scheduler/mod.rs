use std::rc::Rc;

use mavlink::{self, ardupilotmega as apm};
use tokio::{sync::broadcast}

use crate::client::{camera::CameraClient, pixhawk::PixhawkMessage};
use crate::state::RegionOfInterest;

pub mod state;

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
    pixhawk_rx: broadcast::Receiver<PixhawkMessage>,

    /// Channel for communicating with the Camera
    // camera: Rc<CameraClient>,
}

impl Scheduler {
    pub fn new(channels: Arc<Channels>) -> Self {
        Self::with_rois(Vec<RegionOfInterest>::new())
    }

    pub fn with_rois(rois: Vec<RegionOfInterest>, channels: Arc<Channels>) {
        Self {
            rois: rois,
            pixhawk_rx: channels.pixhawk.subscribe(),
        }
    }
}

pub enum SchedulerEvent {
    ROI(RegionOfInterest),
    Coverage,
}
