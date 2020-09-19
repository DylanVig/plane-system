use std::time::{Duration, Instant};

use anyhow::Context;
use smol::net::TcpStream;

use mavlink::{ardupilotmega as apm, common};
use smol_timeout::TimeoutExt;

use super::interface::PixhawkInterface;

pub struct PixhawkClient {
    interface: PixhawkInterface<TcpStream, TcpStream>,
}

impl PixhawkClient {
    pub async fn connect() -> anyhow::Result<Self> {
        let connection = TcpStream::connect("0.0.0.0").await?;
        let interface = PixhawkInterface::new(connection.clone(), connection.clone());
        Ok(PixhawkClient { interface })
    }

    pub async fn init(&mut self) -> anyhow::Result<()> {
        trace!("waiting for heartbeat");
        self.wait_for_message(
            |message| match message {
                apm::MavMessage::common(common::MavMessage::HEARTBEAT(_)) => true,
                _ => false,
            },
            Duration::from_secs(10),
        )
        .await?;
        trace!("received heartbeat");
        trace!("setting parameters");
        self.set_param_f32("CAM_DURATION", 10.0).await?;
        self.set_param_u8("CAM_FEEDBACK_PIN", 54).await?;
        self.set_param_u8("CAM_FEEDBACK_POL", 1).await?;
        self.send_command(
            common::MavCmd::MAV_CMD_DO_DIGICAM_CONTROL,
            [0., 0., 0., 0., 1., 0., 0.],
        )
        .await?;

        Ok(())
    }

    pub async fn run(self) {
        loop {
            // let (_, message) = self.connection.recv()?;

            // debug!("received message: {:?}", message);

            // match &message {
            //     apm::MavMessage::common(common::MavMessage::GLOBAL_POSITION_INT(data)) => {
            //         let gps = PixhawkTelemetryCoords {
            //             // lat and lon are in degrees * 10^7
            //             // altitude is in mm
            //             latitude: data.lat as f32 / 1e7,
            //             longitude: data.lon as f32 / 1e7,
            //             altitude: data.relative_alt as f32 / 1e3,
            //         };

            //         trace!("received global position {:?}", gps);
            //         self.telemetry.write().await.gps = Some(gps);
            //     }
            //     apm::MavMessage::common(common::MavMessage::ATTITUDE(data)) => {
            //         let attitude = PixhawkTelemetryAttitude {
            //             // roll, pitch, yaw are in radians/sec
            //             roll: data.roll as f32,
            //             pitch: data.pitch as f32,
            //             yaw: data.yaw as f32,
            //         };

            //         trace!("received attitude {:?}", attitude);
            //         self.telemetry.write().await.attitude = Some(attitude);
            //     }
            //     apm::MavMessage::CAMERA_FEEDBACK(data) => {
            //         let gps = PixhawkTelemetryCoords {
            //             // lat and lon are in degrees * 10^7
            //             // altitude is in meters
            //             latitude: data.lat as f32 / 1e7,
            //             longitude: data.lng as f32 / 1e7,
            //             altitude: data.alt_rel as f32,
            //         };

            //         trace!("received camera feedback {:?}", gps);
            //         self.telemetry.write().await.gps = Some(gps);
            //     }
            //     _ => {}
            // }

            // self.channels.send_response(message).await?;

            // match self.channels.recv_request().await {
            //   Ok(request) => self.connection.send_default(&request)?,
            //   Err(_) => {}
            // }
        }
    }

    pub async fn wait_for_message<F: Fn(&apm::MavMessage) -> bool>(
        &mut self,
        predicate: F,
        timeout: Duration,
    ) -> anyhow::Result<apm::MavMessage> {
        let deadline = Instant::now() + timeout;

        loop {
            let remaining_time = deadline - Instant::now();

            let message = self.interface.recv().timeout(remaining_time).await;
            let message =
                message.context("Timeout occurred while setting a parameter on the Pixhawk.")?;
            let message = message
                .context("The Pixhawk client closed while setting a parameter on the Pixhawk.")?;

            if predicate(&message) {
                return Ok(message);
            }
        }
    }

    /// Sets a parameter on the Pixhawk and waits for acknowledgement. The
    /// default timeout is 10 seconds.
    pub async fn set_param<T: num_traits::NumCast + std::fmt::Debug>(
        &mut self,
        id: &str,
        param_value: T,
        param_type: common::MavParamType,
    ) -> anyhow::Result<T> {
        trace!("setting param {:?} to {:?}", id, param_value);

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
        self.interface.send(message).await?;

        trace!("sent request, waiting for ack");

        // wait for ack or timeout
        let ack_message = self
            .wait_for_message(
                |message| match message {
                    apm::MavMessage::common(common::MavMessage::PARAM_VALUE(data)) => {
                        data.param_id == param_id
                    }
                    _ => false,
                },
                Duration::from_secs(10),
            )
            .await?;

        match ack_message {
            apm::MavMessage::common(common::MavMessage::PARAM_VALUE(data)) => {
                let param_value = num_traits::cast(data.param_value).unwrap();
                trace!("received ack, current param value is {:?}", param_value);
                Ok(param_value)
            }
            _ => unreachable!(),
        }
    }

    /// Sets a parameter on the Pixhawk and waits for acknowledgement. The
    /// default timeout is 10 seconds.
    pub async fn send_command(
        &mut self,
        command: common::MavCmd,
        params: [f32; 7],
    ) -> anyhow::Result<common::MavResult> {
        trace!("sending command {:?} ({:?})", command, params);

        let message = apm::MavMessage::common(common::MavMessage::COMMAND_LONG(
            common::COMMAND_LONG_DATA {
                command,
                confirmation: 0,
                param1: params[0],
                param2: params[1],
                param3: params[2],
                param4: params[3],
                param5: params[4],
                param6: params[5],
                param7: params[6],
                target_system: 0,
                target_component: 0,
            },
        ));

        // send message
        self.interface.send(message).await?;

        trace!("sent command, waiting for ack");

        // wait for ack or timeout
        let ack_message = self
            .wait_for_message(
                |message| match message {
                    apm::MavMessage::common(common::MavMessage::COMMAND_ACK(data)) => {
                        data.command == command
                    }
                    _ => false,
                },
                Duration::from_secs(10),
            )
            .await?;

        trace!("received ack");

        match ack_message {
            apm::MavMessage::common(common::MavMessage::COMMAND_ACK(data)) => match data.result {
                common::MavResult::MAV_RESULT_ACCEPTED
                | common::MavResult::MAV_RESULT_IN_PROGRESS => Ok(data.result),
                _ => Err(anyhow!(
                    "Command {:?} failed with status code {:?}",
                    command,
                    data.result
                )),
            },
            _ => unreachable!(),
        }
    }

    pub async fn set_param_f32(&mut self, id: &str, value: f32) -> anyhow::Result<f32> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_REAL32)
            .await
    }

    pub async fn set_param_u8(&mut self, id: &str, value: u8) -> anyhow::Result<u8> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_UINT8)
            .await
    }

    pub async fn set_param_i8(&mut self, id: &str, value: i8) -> anyhow::Result<i8> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_INT8)
            .await
    }

    pub async fn set_param_u16(&mut self, id: &str, value: u16) -> anyhow::Result<u16> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_UINT16)
            .await
    }

    pub async fn set_param_i16(&mut self, id: &str, value: i16) -> anyhow::Result<i16> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_INT16)
            .await
    }

    pub async fn set_param_u32(&mut self, id: &str, value: u32) -> anyhow::Result<u32> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_UINT32)
            .await
    }

    pub async fn set_param_i32(&mut self, id: &str, value: i32) -> anyhow::Result<i32> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_INT32)
            .await
    }

    pub async fn set_param_u64(&mut self, id: &str, value: u64) -> anyhow::Result<u64> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_UINT64)
            .await
    }

    pub async fn set_param_i64(&mut self, id: &str, value: i64) -> anyhow::Result<i64> {
        self.set_param(id, value, common::MavParamType::MAV_PARAM_TYPE_INT64)
            .await
    }
}
