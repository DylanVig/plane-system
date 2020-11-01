use crate::{
    state::{
        RegionOfInterest,
        Telemetry,
    },
    scheduler::state::*,
};

use tokio::time::interval;
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct SchedulerBackend {
    /// List of regions of interest that should be photographed as soon as
    /// possible. Scheduler will prioritize attempting to photograph nearby ROIs
    /// over increasing ground coverage.
    rois: Vec<RegionOfInterest>,

    /// The current telemetry that the backend will make base decisions on. The
    /// frontend should update this as it receives new telemetry.
    telemetry: Telemetry,

    /// Bool representing whether it's time to create a capture request.
    time_for_capture: bool,
}


impl SchedulerBackend {
    pub fn new() -> Self {
        Self {
            rois: Vec::new(),
            telemetry: Telemetry::default(),
            time_for_capture: true,            
        }
    }

    pub fn update_telemetry(&mut self, telemetry: Telemetry) {
        self.telemetry = telemetry;
    }

    pub fn get_capture_request(&mut self) -> Option<CaptureRequest> {
        if self.time_for_capture {
            self.time_for_capture = false;
            return Some(CaptureRequest::from_capture_type(CaptureType::Fixed));
        }
        None
    }

    pub fn set_capture_response(&mut self) {
        self.time_for_capture = true;
    }
}
