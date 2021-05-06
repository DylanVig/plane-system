use crate::{
    scheduler::state::*,
    state::{Coords2D, RegionOfInterest, TelemetryInfo},
};

use geo::{
    algorithm::{bearing::Bearing, haversine_distance::HaversineDistance},
    Point,
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

    /// Temporary hack for test flight purposes.
    gps: Coords2D,
}

impl SchedulerBackend {
    pub fn new(gps: Coords2D) -> Self {
        Self {
            rois: Vec::new(),
            telemetry: TelemetryInfo::default(),
            time_for_capture: true,
            gps,
        }
    }

    pub fn update_telemetry(&mut self, telemetry: TelemetryInfo) {
        self.telemetry = telemetry;
    }

    pub fn get_capture_request(&mut self) -> Option<CaptureRequest> {
        if self.time_for_capture {
            self.time_for_capture = false;
            return Some(CaptureRequest::from_capture_type(CaptureType::Fixed));
        }
        None
    }

    pub fn get_target_gimbal_angles(&mut self) -> (f64, f64) {
        // altitude in m, no conversion needed
        let altitude = self.telemetry.position.altitude_rel as f64;

        // roll, pitch, yaw in degrees, need radians
        let plane_roll = self.telemetry.plane_attitude.roll.to_radians() as f64;
        let plane_pitch = self.telemetry.plane_attitude.pitch.to_radians() as f64;
        let plane_yaw = self.telemetry.plane_attitude.yaw.to_radians() as f64;

        // next we need to get the distance from the plane to the gps location
        let current_loc = Point::<f64>::new(
            self.telemetry.position.longitude as f64,
            self.telemetry.position.latitude as f64,
        );
        let gps_loc = Point::<f64>::new(self.gps.longitude as f64, self.gps.latitude as f64);

        // distance is given in m, no conversion needed
        let distance = current_loc.haversine_distance(&gps_loc);
        // bearing given in degrees, convert to radians. pretty sure it's relative to and which direction the bearing increases
        // assuming relative to north and increases clockwise
        let bearing = current_loc.bearing(gps_loc).to_radians();

        // distance and bearing form a vector, first get x,y components relative to world
        // x_world is east, y_world is north
        let vec_x_world = distance * bearing.sin();
        let vec_y_world = distance * bearing.cos();

        // then we convert these to the plane's reference frame
        // x_plane is right, y_plane is forward
        let vec_x_plane = vec_x_world * plane_yaw.cos() - vec_y_world * plane_yaw.sin();
        let vec_y_plane = vec_x_world * plane_yaw.sin() + vec_y_world * plane_yaw.cos();

        // we also compute the z vector, which is pointing straight up
        let vec_z_plane = altitude;

        // we now have all the data to compute the angles
        let roll = (-vec_x_plane).atan2(vec_z_plane).to_degrees();
        // TODO go back to this
        let pitch = (-vec_y_plane)
            .atan2((vec_z_plane * vec_z_plane + vec_x_plane * vec_x_plane).sqrt())
            .to_degrees();
        trace!("roll: {:?}, pitch: {:?}", roll, pitch);
        return (roll, pitch);
    }

    pub fn set_capture_response(&mut self) {
        self.time_for_capture = true;
    }
}
