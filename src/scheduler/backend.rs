use crate::{
    scheduler::state::*,
    state::{RegionOfInterest, TelemetryInfo},
};

pub struct SchedulerBackend {
    /// List of regions of interest that should be photographed as soon as
    /// possible. Scheduler will prioritize attempting to photograph nearby ROIs
    /// over increasing ground coverage.
    rois: Vec<RegionOfInterest>,

    /// The current telemetry that the backend will make base decisions on. The
    /// frontend should update this as it receives new telemetry.
    telemetry: TelemetryInfo,

    /// Bool representing whether it's time to create a capture request.
    time_for_capture: bool,

    test_counter: i16,

    test_sign: i16,
}

impl SchedulerBackend {
    pub fn new() -> Self {
        Self {
            rois: Vec::new(),
            telemetry: TelemetryInfo::default(),
            time_for_capture: true,
            test_counter: 0,
            test_sign: 1,
        }
    }

    pub fn update_telemetry(&mut self, telemetry: TelemetryInfo) {
        self.telemetry = telemetry;
        self.test_counter += 1;
        if self.test_counter == 250 {
            self.test_counter = 0;
            self.test_sign *= -1;
        }
    }

    pub fn get_capture_request(&mut self) -> Option<CaptureRequest> {
        if self.time_for_capture {
            self.time_for_capture = false;
            return Some(CaptureRequest::from_capture_type(CaptureType::Fixed));
        }
        None
    }

    pub fn get_target_gimbal_angles(&mut self) -> (i16, i16) {
        return (self.test_sign * self.test_counter / 5, self.test_sign * -self.test_counter / 5)
    }

    pub fn set_capture_response(&mut self) {
        self.time_for_capture = true;
    }
}
