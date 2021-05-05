use num_traits::FromPrimitive;
use serde::Deserialize;

// real gimbal
pub mod hardware;

// virtual gimbal
pub mod software;

pub use hardware::*;
pub use software::*;

use simplebgc::*;

use crate::state::TelemetryInfo;

#[derive(Copy, Clone, Eq, PartialEq, Debug, Deserialize)]
pub enum GimbalProtocol {
    SimpleBGC,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Deserialize)]
#[serde(tag = "type")]
pub enum GimbalKind {
    Hardware { protocol: GimbalProtocol },
    Software,
}

#[async_trait]
pub trait GimbalInterface: Send {
    async fn control_angles(&mut self, roll: f64, pitch: f64) -> anyhow::Result<()>;
    async fn point_at_gps(
        &mut self,
        lat: f32,
        lon: f32,
        telem: &Option<TelemetryInfo>,
    ) -> anyhow::Result<()>;
}

#[async_trait]
pub trait SimpleBgcGimbalInterface: GimbalInterface {
    async fn send_command(&mut self, cmd: OutgoingCommand) -> anyhow::Result<()>;

    async fn recv_command(&mut self) -> anyhow::Result<Option<IncomingCommand>>;

}

#[async_trait]
impl <T: SimpleBgcGimbalInterface> GimbalInterface for T {
    async fn control_angles(&mut self, roll: f64, pitch: f64) -> anyhow::Result<()> {
        let factor: f64 = (2 ^ 14) as f64 / 360.0;

        let command = OutgoingCommand::Control(ControlData {
            mode: ControlFormat::Legacy(AxisControlState::from_u8(0x02).unwrap()),
            axes: RollPitchYaw {
                roll: AxisControlParams {
                    /// unit conversion: SBGC units are 360 / 2^14 degrees
                    angle: (roll * factor) as i16,
                    speed: 1200,
                },
                pitch: AxisControlParams {
                    /// unit conversion: SBGC units are 360 / 2^14 degrees
                    angle: (pitch * factor) as i16,
                    speed: 2400,
                },
                yaw: AxisControlParams { angle: 0, speed: 0 },
            },
        });

        self.send_command(command).await?;

        Ok(())
    }

    async fn point_at_gps(
        &mut self,
        lat: f32,
        lon: f32,
        telem: &Option<TelemetryInfo>,
    ) -> anyhow::Result<()> {
        let mut roll: f64 = 0.;
        let mut pitch: f64 = 0.;
        if let Some(telem) = telem {
            let norm = |a: f32, b: f32| f32::sqrt(a * a + b * b);
            let cos = f32::cos;
            let sin = f32::sin;

            let plane_roll = telem.plane_attitude.roll;
            let plane_pitch = telem.plane_attitude.pitch;
            let plane_yaw = telem.plane_attitude.yaw;

            let plane_lat = telem.position.latitude;
            let plane_lon = telem.position.longitude;
            let plane_alt = telem.position.altitude;

            let cos_yaw = cos(plane_yaw);
            let sin_yaw = sin(plane_yaw);

            // All values in CMs
            let gps_vector_x_world = 100.0
                * (lon - plane_lon)
                * cos(((plane_lat + lat) * 0.00000005).to_radians())
                * 0.01113195;
            let gps_vector_y_world = 100.0 * (lat - plane_lat) * 0.01113195;
            let gps_vector_z = -plane_alt;

            // Getting x, y vectors in plane's reference plane
            let gps_vector_x = gps_vector_x_world * cos_yaw - gps_vector_y_world * sin_yaw;
            let gps_vector_y = gps_vector_x_world * sin_yaw + gps_vector_y_world * cos_yaw;

            if (gps_vector_z != 0.0) {
                roll = -gps_vector_x.atan2(gps_vector_z) as f64;
            }

            // Pitch
            if (norm(gps_vector_z, gps_vector_x) != 0.0) {
                pitch = gps_vector_y.atan2(norm(gps_vector_z, gps_vector_x)) as f64;
            }
        }
        self.control_angles(roll, pitch).await
    }
}
