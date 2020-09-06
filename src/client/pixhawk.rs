use mavlink::{self, ardupilotmega, common};
use smol::{
    channel::{Receiver, Sender},
    lock::RwLock,
};
use std::{future::Future, pin::Pin, sync::Arc};

pub struct PixhawkClient {
    telemetry: Arc<RwLock<PixhawkTelemetry>>,
    message_receiver: Receiver<ardupilotmega::MavMessage>,
    message_sender: Sender<ardupilotmega::MavMessage>,
    message_loop: Pin<Box<dyn Future<Output = anyhow::Result<()>>>>,
}

#[derive(Debug)]
pub struct PixhawkTelemetry {
    gps: Option<PixhawkTelemetryCoords>,
    attitude: Option<PixhawkTelemetryAttitude>,
    geotag: Option<PixhawkTelemetryCoords>,
}

#[derive(Debug)]
pub struct PixhawkTelemetryCoords {
    latitude: f64,
    longitude: f64,
    altitude: f64,
}

#[derive(Debug)]
pub struct PixhawkTelemetryAttitude {
    roll: f64,
    pitch: f64,
    yaw: f64,
}

impl PixhawkClient {
    pub fn connect(address: &str) -> anyhow::Result<Self> {
        // channel for distributing messages we received from pixhawk
        let (message_broadcaster, message_receiver) = smol::channel::unbounded();

        // channel for sending messages back to the pixhawk
        let (message_sender, message_terminal) = smol::channel::unbounded();

        let connection = mavlink::connect(address)?;
        let telemetry = Arc::new(RwLock::new(PixhawkTelemetry {
            gps: None,
            attitude: None,
            geotag: None,
        }));

        let client = PixhawkClient {
            telemetry: telemetry.clone(),
            message_receiver,
            message_sender,
            message_loop: Box::pin(async move {
                loop {
                    let (_, message) = connection.recv()?;

                    match &message {
                        ardupilotmega::MavMessage::common(
                            common::MavMessage::GLOBAL_POSITION_INT(data),
                        ) => {
                            let gps = PixhawkTelemetryCoords {
                                // lat and lon are in degrees * 10^7
                                // altitude is in mm
                                latitude: data.lat as f64 / 1e7,
                                longitude: data.lon as f64 / 1e7,
                                altitude: data.relative_alt as f64 / 1e3,
                            };

                            trace!("received global position {:?}", gps);
                            telemetry.write().await.gps = Some(gps);
                        }
                        ardupilotmega::MavMessage::common(common::MavMessage::ATTITUDE(data)) => {
                            let attitude = PixhawkTelemetryAttitude {
                                // lat and lon are in degrees * 10^7
                                // altitude is in mm
                                roll: data.roll as f64 / 1e7,
                                pitch: data.pitch as f64 / 1e7,
                                yaw: data.yaw as f64 / 1e3,
                            };

                            trace!("received attitude {:?}", attitude);
                            telemetry.write().await.attitude = Some(attitude);
                        }
                        ardupilotmega::MavMessage::CAMERA_FEEDBACK(data) => {
                            let gps = PixhawkTelemetryCoords {
                                // lat and lon are in degrees * 10^7
                                // altitude is in meters
                                latitude: data.lat as f64 / 1e7,
                                longitude: data.lng as f64 / 1e7,
                                altitude: data.alt_rel as f64,
                            };

                            trace!("received camera feedback {:?}", gps);
                            telemetry.write().await.gps = Some(gps);
                        }
                        _ => {}
                    }

                    while !message_terminal.is_empty() {
                        let message = message_terminal.recv().await?;
                        connection.send_default(&message)?;
                    }

                    message_broadcaster.send(message).await?;
                }

                Ok(())
            }),
        };

        Ok(client)
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.message_loop.as_mut().await
    }

    pub async fn set_param(&self)
  }
