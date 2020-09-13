use mavlink::{self, ardupilotmega as apm, common, MavConnection};
use smol::lock::RwLock;

use crate::client::{Channels, pixhawk::PixhawkClient};

#[derive(Debug)]
pub struct PixhawkTelemetry {
    gps: Option<PixhawkTelemetryCoords>,
    attitude: Option<PixhawkTelemetryAttitude>,
    geotag: Option<PixhawkTelemetryCoords>,
}

#[derive(Debug)]
pub struct PixhawkTelemetryCoords {
    latitude: f32,
    longitude: f32,
    altitude: f32,
}

#[derive(Debug)]
pub struct PixhawkTelemetryAttitude {
    roll: f32,
    pitch: f32,
    yaw: f32,
}

pub struct PixhawkInterface {
    connection: Box<dyn MavConnection<apm::MavMessage> + Send>,
    telemetry: RwLock<PixhawkTelemetry>,
    channels: Channels<apm::MavMessage, apm::MavMessage>,
}

impl PixhawkInterface {
    /// Connects to the Pixhawk at the given address. Should be formatted as a
    /// Mavlink address, i.e. `tcpin:192.168.4.4`
    pub fn connect(address: &str) -> anyhow::Result<Self> {
        let connection = mavlink::connect(address)?;
        let telemetry = RwLock::new(PixhawkTelemetry {
            gps: None,
            attitude: None,
            geotag: None,
        });

        let interface = PixhawkInterface {
            connection,
            telemetry,
            channels: Channels::new(),
        };

        Ok(interface)
    }

    pub fn new_client(&self) -> PixhawkClient {
        PixhawkClient {
            channels: self.channels.clone(),
        }
    }

    /// Starts a task that will run the Pixhawk.
    pub fn run(self) -> smol::Task<anyhow::Result<()>> {
        smol::spawn(async move {
            loop {
                let (_, message) = self.connection.recv()?;

                debug!("received message: {:?}", message);

                match &message {
                    apm::MavMessage::common(common::MavMessage::GLOBAL_POSITION_INT(data)) => {
                        let gps = PixhawkTelemetryCoords {
                            // lat and lon are in degrees * 10^7
                            // altitude is in mm
                            latitude: data.lat as f32 / 1e7,
                            longitude: data.lon as f32 / 1e7,
                            altitude: data.relative_alt as f32 / 1e3,
                        };

                        trace!("received global position {:?}", gps);
                        self.telemetry.write().await.gps = Some(gps);
                    }
                    apm::MavMessage::common(common::MavMessage::ATTITUDE(data)) => {
                        let attitude = PixhawkTelemetryAttitude {
                            // roll, pitch, yaw are in radians/sec
                            roll: data.roll as f32,
                            pitch: data.pitch as f32,
                            yaw: data.yaw as f32,
                        };

                        trace!("received attitude {:?}", attitude);
                        self.telemetry.write().await.attitude = Some(attitude);
                    }
                    apm::MavMessage::CAMERA_FEEDBACK(data) => {
                        let gps = PixhawkTelemetryCoords {
                            // lat and lon are in degrees * 10^7
                            // altitude is in meters
                            latitude: data.lat as f32 / 1e7,
                            longitude: data.lng as f32 / 1e7,
                            altitude: data.alt_rel as f32,
                        };

                        trace!("received camera feedback {:?}", gps);
                        self.telemetry.write().await.gps = Some(gps);
                    }
                    _ => {}
                }

                self.channels.send_response(message).await?;

                match self.channels.recv_request().await {
                  Ok(request) => self.connection.send_default(&request)?,
                  Err(_) => {}
                }
            }
        })
    }
}
