use std::rc::Rc;

use mavlink::{self, ardupilotmega as apm};
use smol::channel::{Receiver, Sender};
use state::RegionOfInterest;

use crate::client::{pixhawk::PixhawkClient, camera::CameraClient};

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

    /// Channel for communicating with the Pixhawk.
    pixhawk: Rc<PixhawkClient>,

    /// Channel for communicating with the Camera
    camera: Rc<CameraClient>
}

pub enum SchedulerEvent {
  ROI(RegionOfInterest),
  Coverage
}
