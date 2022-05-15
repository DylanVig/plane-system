use std::{
    sync::atomic::AtomicU8,
    sync::atomic::Ordering,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use anyhow::Context;
use bytes::{Buf, BytesMut};

use tokio::net::ToSocketAddrs;

use mavlink::{
    ardupilotmega as apm, common, error::MessageReadError, error::ParserError, MavHeader,
    MavlinkVersion,
};

use crate::{
    state::{Attitude, Point3D},
    util::run_loop,
    Channels,
};

use super::{state::PixhawkEvent, PixhawkCommand};

pub struct PixhawkClient {
    sock: tokio::net::UdpSocket,
    buf: BytesMut,
    sequence: AtomicU8,
    channels: Arc<Channels>,
    cmd: flume::Receiver<PixhawkCommand>,
    version: MavlinkVersion,
}

impl PixhawkClient {
    pub async fn connect<A: ToSocketAddrs + Clone>(
        channels: Arc<Channels>,
        cmd: flume::Receiver<PixhawkCommand>,
        addr: A,
        version: MavlinkVersion,
    ) -> anyhow::Result<Self> {
        let sock = tokio::net::UdpSocket::bind(addr)
            .await
            .context("failed to connect to pixhawk")?;

        debug!("waiting for packet from mavproxy");

        let (_, remote_addr) =
            tokio::time::timeout(Duration::from_secs(60), sock.recv_from(&mut []))
                .await
                .context("timed out while waiting for packet from mavproxy")?
                .context("error retrieving packet from mavproxy")?;

        info!(
            "received packet from {:?}, locking to this address",
            remote_addr
        );

        sock.connect(remote_addr)
            .await
            .context("failed to lock to address")?;

        match version {
            MavlinkVersion::V1 => debug!("using mavlink v1"),
            MavlinkVersion::V2 => debug!("using mavlink v2"),
        };

        Ok(PixhawkClient {
            sock,
            buf: BytesMut::with_capacity(1024),
            sequence: AtomicU8::default(),
            channels,
            cmd,
            version,
        })
    }

    pub async fn init(&mut self) -> anyhow::Result<()> {
        info!("waiting for heartbeat");
        self.wait_for_message(
            |message| match message {
                apm::MavMessage::common(common::MavMessage::HEARTBEAT(_)) => true,
                _ => false,
            },
            Duration::from_secs(100),
        )
        .await
        .context("waiting for heartbeat")?;

        info!("received heartbeat");
        info!("setting parameters");

        self.set_param_f32("CAM_DURATION", 10.0).await?;
        self.set_param_u8("CAM_FEEDBACK_PIN", 54).await?;
        self.set_param_u8("CAM_FEEDBACK_POL", 1).await?;
        self.send_command(
            common::MavCmd::MAV_CMD_DO_DIGICAM_CONTROL,
            [0., 0., 0., 0., 1., 0., 0.],
        )
        .await?;
        self.send_command(
            common::MavCmd::MAV_CMD_SET_MESSAGE_INTERVAL,
            [33., 1000., 0., 0., 0., 0., 0.],
        )
        .await?;
        self.send_command(
            common::MavCmd::MAV_CMD_SET_MESSAGE_INTERVAL,
            [30., 1000., 0., 0., 0., 0., 0.],
        )
        .await?;

        info!("finished initialization");

        Ok(())
    }

    /// Sends a message to the Pixhawk.
    pub async fn send(&mut self, message: apm::MavMessage) -> anyhow::Result<()> {
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);

        debug!("sending message: {:?}", &message);

        let header = MavHeader {
            sequence,
            system_id: 1,
            component_id: 1,
        };

        let mut buf = Vec::with_capacity(1024);

        mavlink::write_versioned_msg(&mut buf, self.version, header, &message)?;
        self.sock.send(buf.as_ref()).await?;

        Ok(())
    }

    /// Waits for a message from the Pixhawk, reacts to it, and returns it.
    pub async fn recv(&mut self) -> anyhow::Result<apm::MavMessage> {
        loop {
            let mut chunk = vec![0; 1024];

            let magic = match self.version {
                MavlinkVersion::V1 => 0xFE,
                MavlinkVersion::V2 => 0xFD,
            };

            trace!("buf is {:?} bytes long", self.buf.len());

            let magic_position = loop {
                let magic_position = self.buf.iter().position(|&b| b == magic);

                match magic_position {
                    // we need at least two bytes after the magic in the buffer
                    Some(magic_position) if magic_position + 2 < self.buf.len() => {
                        break magic_position
                    }
                    res => {
                        trace!("requesting more bytes, magic too close to end ({:?})", res);

                        let (n, addr) = self.sock.recv_from(&mut chunk[..]).await?;
                        self.buf.extend(&chunk[..n]);
                        trace!("read {:?} bytes from {:?}", n, addr);
                    }
                };
            };

            trace!(
                "found magic at position {:?} in buf length {:?}",
                magic_position,
                self.buf.len()
            );

            let payload_len = self.buf[magic_position + 1];

            let msg_body_size = match self.version {
                // in v1: 1 byte magic + 1 byte payload len + 4 byte header + 2 byte checksum
                MavlinkVersion::V1 => payload_len as usize + 8,
                // in v2: 1 byte magic + 1 byte payload len + 8 byte header + 2 byte checksum
                MavlinkVersion::V2 => payload_len as usize + 12,
            };

            trace!("need {:?} bytes", msg_body_size);

            while magic_position + msg_body_size > self.buf.len() {
                trace!("requesting more bytes, buffer insufficient");

                let mut chunk = vec![0; 1024];
                let (n, addr) = self.sock.recv_from(&mut chunk[..]).await?;
                self.buf.extend(&chunk[..n]);
                trace!("read {:?} bytes from {:?}", n, addr);
            }

            let msg_content = &self.buf[magic_position..magic_position + msg_body_size];

            // if we get a bad checksum, just drop the message and try again
            let msg = match mavlink::read_versioned_msg(&mut &msg_content[..], self.version) {
                Ok((_, msg)) => {
                    let skip = magic_position + msg_body_size;
                    trace!("parsed message, success, skipping {:?} bytes", skip);
                    self.buf.advance(skip);
                    msg
                }
                Err(MessageReadError::Parse(ParserError::InvalidChecksum { .. })) => {
                    trace!("got invalid checksum, dropping message");
                    let skip = magic_position + 1;
                    self.buf.advance(skip);
                    continue;
                }
                Err(err) => return Err(err).context("error while parsing message"),
            };

            trace!("received message: {:?}", msg);

            self.handle(&msg).await?;

            return Ok(msg);
        }
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        info!("initializing pixhawk");

        self.init().await?;

        let mut interrupt_recv = self.channels.interrupt.subscribe();

        run_loop!(
            async move {
                // no delay b/c this is an I/O-bound loop
                loop {
                    if let Ok(cmd) = self.cmd.try_recv() {
                        self.exec(cmd).await?;
                    }

                    let _ = self.recv().await?;
                }
            },
            interrupt_recv.recv()
        );

        Ok(())
    }

    async fn exec(&mut self, _cmd: PixhawkCommand) -> anyhow::Result<()> {
        unimplemented!()
    }

    /// Reacts to a message received from the Pixhawk.
    async fn handle(&self, message: &apm::MavMessage) -> anyhow::Result<()> {
        match message {
            apm::MavMessage::common(common::MavMessage::GLOBAL_POSITION_INT(data)) => {
                let _ = self.channels.pixhawk_event.send(PixhawkEvent::Gps {
                    position: Point3D {
                        point: geo::Point::new(data.lon as f32 / 1e7, data.lat as f32 / 1e7),
                        altitude_msl: data.alt as f32 / 1e3,
                        altitude_rel: data.relative_alt as f32 / 1e3,
                    },
                    // velocity is provided as (North, East, Down)
                    // so we transform it to more common (East, North, Up)
                    velocity: (
                        data.vy as f32 / 100.,
                        data.vx as f32 / 100.,
                        -data.vz as f32 / 100.,
                    ),
                });
            }
            apm::MavMessage::common(common::MavMessage::ATTITUDE(data)) => {
                let _ = self.channels.pixhawk_event.send(PixhawkEvent::Orientation {
                    attitude: Attitude::new(
                        data.roll.to_degrees(),
                        data.pitch.to_degrees(),
                        data.yaw.to_degrees(),
                    ),
                });
            }
            apm::MavMessage::CAMERA_FEEDBACK(data) => {
                let _ = self.channels.pixhawk_event.send(PixhawkEvent::Image {
                    foc_len: data.foc_len,
                    img_idx: data.img_idx,
                    cam_idx: data.cam_idx,
                    flags: data.flags,
                    time: SystemTime::UNIX_EPOCH + Duration::from_micros(data.time_usec),
                    attitude: Attitude::new(data.roll, data.pitch, data.yaw),
                    coords: Point3D {
                        point: geo::Point::new(data.lng as f32 / 1e7, data.lat as f32 / 1e7),
                        altitude_msl: data.alt_msl,
                        altitude_rel: data.alt_rel,
                    },
                });
            }
            _ => {}
        }

        Ok(())
    }

    pub async fn wait_for_message<F: Fn(&apm::MavMessage) -> bool>(
        &mut self,
        predicate: F,
        timeout: Duration,
    ) -> anyhow::Result<apm::MavMessage> {
        let deadline = Instant::now() + timeout;

        loop {
            let remaining_time = deadline - Instant::now();

            let message = tokio::time::timeout(remaining_time, self.recv()).await;
            let message = message
                .context("Timeout occurred while waiting for a message from the Pixhawk.")?;
            let message =
                message.context("Error occurred while reading a message from the Pixhawk.")?;

            if predicate(&message) {
                return Ok(message);
            }
        }
    }

    pub async fn ping(&mut self) -> anyhow::Result<()> {
        debug!("pinging pixhawk");

        let message = apm::MavMessage::common(common::MavMessage::PING(common::PING_DATA {
            time_usec: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            seq: 0,
            target_system: 0,
            target_component: 0,
        }));

        self.send(message).await?;

        self.wait_for_message(
            |message| match message {
                apm::MavMessage::common(common::MavMessage::PING(_)) => {
                    debug!("received ping back");
                    true
                }
                _ => false,
            },
            Duration::from_secs(10),
        )
        .await?;

        Ok(())
    }

    /// Sets a parameter on the Pixhawk and waits for acknowledgement. The
    /// default timeout is 10 seconds.
    pub async fn set_param<T: num_traits::NumCast + std::fmt::Debug>(
        &mut self,
        id: &str,
        param_value: T,
        param_type: common::MavParamType,
    ) -> anyhow::Result<T> {
        debug!("setting param {:?} to {:?}", id, param_value);

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
        self.send(message).await?;

        debug!("sent request, waiting for ack");

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
            .await
            .context("Error occurred while waiting for ack after setting parameter")?;

        match ack_message {
            apm::MavMessage::common(common::MavMessage::PARAM_VALUE(data)) => {
                let param_value = num_traits::cast(data.param_value).unwrap();
                debug!("received ack, current param value is {:?}", param_value);
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
        debug!("sending command {:?} ({:?})", command, params);

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
        self.send(message).await?;

        debug!("sent command, waiting for ack");

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

        debug!("received ack");

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
