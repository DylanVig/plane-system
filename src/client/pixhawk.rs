use mavlink::{self, ardupilotmega as apm, common, MavConnection};
use num_traits;
use smol::{
    channel::{Receiver, Sender},
    lock::RwLock,
};
use std::sync::Arc;

pub struct PixhawkClient {
    connection: Box<dyn MavConnection<apm::MavMessage>>,
    telemetry: Arc<RwLock<PixhawkTelemetry>>,
    message_send_channel: (Sender<apm::MavMessage>, Receiver<apm::MavMessage>),
    message_recv_channel: (Sender<apm::MavMessage>, Receiver<apm::MavMessage>),
}

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

impl PixhawkClient {
    /// Connects to the Pixhawk at the given address. Should be formatted as a
    /// Mavlink address, i.e. `tcpin:192.168.4.4`
    pub fn connect(address: &str) -> anyhow::Result<Self> {
        // channel for distributing messages we received from pixhawk
        let message_recv_channel = smol::channel::unbounded();

        // channel for sending messages back to the pixhawk
        let message_send_channel = smol::channel::unbounded();

        let connection = mavlink::connect(address)?;
        let telemetry = Arc::new(RwLock::new(PixhawkTelemetry {
            gps: None,
            attitude: None,
            geotag: None,
        }));

        let client = PixhawkClient {
            connection,
            telemetry,
            message_send_channel,
            message_recv_channel,
        };

        Ok(client)
    }

    /// Gets a sender that can send messages to the Pixhawk.
    fn sender(&self) -> Sender<apm::MavMessage> {
        let (message_sender, _) = &self.message_send_channel;
        message_sender.clone()
    }

    /// Gets a receiver that can receive messages from the Pixhawk.
    fn receiver(&self) -> Receiver<apm::MavMessage> {
        let (_, message_receiver) = &self.message_recv_channel;
        message_receiver.clone()
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let (message_broadcaster, _) = &self.message_recv_channel;
        let (_, message_terminal) = &self.message_send_channel;

        loop {
            let (_, message) = self.connection.recv()?;

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

            message_broadcaster.send(message).await?;

            while !message_terminal.is_empty() {
                let message = message_terminal.recv().await?;
                self.connection.send_default(&message)?;
            }
        }
    }

    async fn set_param<T: num_traits::NumCast>(
        &self,
        id: &str,
        param_value: T,
        param_type: common::MavParamType,
    ) -> anyhow::Result<()> {
        let mut param_id: [char; 16] = ['\0'; 16];
        for (index, character) in id.char_indices() {
            param_id[index] = character;
        }

        let message =
            apm::MavMessage::common(common::MavMessage::PARAM_SET(common::PARAM_SET_DATA {
                param_id,
                param_type,
                param_value: num_traits::cast(param_value).unwrap(),
                target_system: 0,
                target_component: 0,
            }));

        // send message
        let sender = self.sender();
        sender.send(message).await?;

        Ok(())
    }

    pub async fn set_param_f32(&self, id: &str, value: f32) -> anyhow::Result<()> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_REAL32)
            .await
    }

    pub async fn set_param_u8(&self, id: &str, value: u8) -> anyhow::Result<()> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_UINT8)
            .await
    }

    pub async fn set_param_i8(&self, id: &str, value: i8) -> anyhow::Result<()> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_INT8)
            .await
    }

    pub async fn set_param_u16(&self, id: &str, value: u16) -> anyhow::Result<()> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_UINT16)
            .await
    }

    pub async fn set_param_i16(&self, id: &str, value: i16) -> anyhow::Result<()> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_INT16)
            .await
    }

    pub async fn set_param_u32(&self, id: &str, value: u32) -> anyhow::Result<()> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_UINT32)
            .await
    }

    pub async fn set_param_i32(&self, id: &str, value: i32) -> anyhow::Result<()> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_INT32)
            .await
    }

    pub async fn set_param_u64(&self, id: &str, value: u64) -> anyhow::Result<()> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_UINT64)
            .await
    }

    pub async fn set_param_i64(&self, id: &str, value: i64) -> anyhow::Result<()> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_INT64)
            .await
    }
}
