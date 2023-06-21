use anyhow::Context;
use bytes::{Buf, BytesMut};
use tracing::*;
use std::{
    sync::atomic::{AtomicU8, Ordering},
    time::{Duration, Instant, SystemTime},
};

use tokio::net::ToSocketAddrs;

use mavlink::{
    ardupilotmega as apm, common, error::MessageReadError, error::ParserError, MavHeader,
    MavlinkVersion,
};

pub struct PixhawkInterface {
    sock: tokio::net::UdpSocket,
    seq_num: Option<u8>,
    buf: BytesMut,
    sequence: AtomicU8,
    version: MavlinkVersion,
}

impl PixhawkInterface {
    pub async fn connect<A: ToSocketAddrs + Clone>(
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

        Ok(PixhawkInterface {
            sock,
            seq_num: None,
            buf: BytesMut::with_capacity(1024),
            sequence: AtomicU8::default(),
            version,
        })
    }

    pub async fn init(&mut self) -> anyhow::Result<()> {
        info!("waiting for heartbeat");
        self.wait_for_message(
            |message| match message {
                apm::MavMessage::HEARTBEAT(_) => true,
                _ => false,
            },
            Duration::from_secs(100),
        )
        .await
        .context("waiting for heartbeat")?;

        info!("received heartbeat");
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

            let seq_num = self.buf[magic_position + 4];

            if let Some(prev_seq_num) = &mut self.seq_num {
                let expected_seq_num = prev_seq_num.wrapping_add(1);

                if expected_seq_num != seq_num {
                    debug!("unexpected sequence number {seq_num} (wanted {expected_seq_num}) for pixhawk packet, assuming packet loss");
                    let skip = magic_position + 1;
                    trace!("skipping forward {skip} bytes");
                    self.buf.advance(skip);
                    continue;
                } else {
                    *prev_seq_num = seq_num;
                }
            } else {
                self.seq_num = Some(seq_num);
            }

            trace!("seq num = {seq_num}");

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
                // Err(MessageReadError::Parse(ParserError::InvalidChecksum { .. })) => {
                //     debug!(
                //         "message parsing failure (invalid checksum); buffer contents: {:02x?}",
                //         msg_content,
                //     );
                //     trace!("got invalid checksum, dropping message");
                //     let skip = magic_position + msg_body_size;
                //     // let skip = magic_position + 1;
                //     self.buf.advance(skip);
                //     continue;
                // }
                Err(err) => {
                    warn!(
                        "message parsing failure ({:?}); buffer contents: {:02x?}",
                        err, msg_content
                    );
                    return Err(err).context("error while parsing message");
                }
            };

            trace!("received message: {:?}", msg);

            return Ok(msg);
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

    #[allow(dead_code)]
    pub async fn ping(&mut self) -> anyhow::Result<()> {
        debug!("pinging pixhawk");

        let message = apm::MavMessage::PING(apm::PING_DATA {
            time_usec: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            seq: 0,
            target_system: 0,
            target_component: 0,
        });

        self.send(message).await?;

        self.wait_for_message(
            |message| match message {
                apm::MavMessage::PING(_) => {
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
    #[allow(dead_code)]
    pub async fn set_param<T: MavParam>(
        &mut self,
        id: &str,
        param_value: T,
    ) -> anyhow::Result<T> {
        debug!("setting param {:?} to {:?}", id, param_value);

        let mut param_id: [u8; 16] = [0; 16];
        for (index, byte) in id.as_bytes().iter().enumerate() {
            param_id[index] = *byte;
        }

        let message = apm::MavMessage::PARAM_SET(apm::PARAM_SET_DATA {
            param_id,
            param_type: T::MAV_PARAM_TYPE,
            param_value: num_traits::cast(param_value).unwrap(),
            target_system: 0,
            target_component: 0,
        });

        // send message
        self.send(message).await?;

        debug!("sent request, waiting for ack");

        // wait for ack or timeout
        let ack_message = self
            .wait_for_message(
                |message| match message {
                    apm::MavMessage::PARAM_VALUE(data) => data.param_id == param_id,
                    _ => false,
                },
                Duration::from_secs(10),
            )
            .await
            .context("Error occurred while waiting for ack after setting parameter")?;

        match ack_message {
            apm::MavMessage::PARAM_VALUE(data) => {
                let param_value = num_traits::cast(data.param_value).unwrap();
                debug!("received ack, current param value is {:?}", param_value);
                Ok(param_value)
            }
            _ => unreachable!(),
        }
    }

    /// Sets a parameter on the Pixhawk and waits for acknowledgement. The
    /// default timeout is 10 seconds.
    #[allow(dead_code)]
    pub async fn send_command(
        &mut self,
        command: apm::MavCmd,
        params: [f32; 7],
    ) -> anyhow::Result<apm::MavResult> {
        debug!("sending command {:?} ({:?})", command, params);

        let message = apm::MavMessage::COMMAND_LONG(apm::COMMAND_LONG_DATA {
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
        });

        // send message
        self.send(message).await?;

        debug!("sent command, waiting for ack");

        // wait for ack or timeout
        let ack_message = self
            .wait_for_message(
                |message| match message {
                    apm::MavMessage::COMMAND_ACK(data) => data.command == command,
                    _ => false,
                },
                Duration::from_secs(10),
            )
            .await?;

        debug!("received ack");

        match ack_message {
            apm::MavMessage::COMMAND_ACK(data) => match data.result {
                apm::MavResult::MAV_RESULT_ACCEPTED | apm::MavResult::MAV_RESULT_IN_PROGRESS => {
                    Ok(data.result)
                }
                _ => Err(anyhow::anyhow!(
                    "Command {:?} failed with status code {:?}",
                    command,
                    data.result
                )),
            },
            _ => unreachable!(),
        }
    }
}

pub trait MavParam: num_traits::NumCast + std::fmt::Debug {
    const MAV_PARAM_TYPE: apm::MavParamType;
}

impl MavParam for f32 {
    const MAV_PARAM_TYPE: apm::MavParamType = apm::MavParamType::MAV_PARAM_TYPE_REAL32;
}

impl MavParam for f64 {
    const MAV_PARAM_TYPE: apm::MavParamType = apm::MavParamType::MAV_PARAM_TYPE_REAL64;
}

impl MavParam for u8 {
    const MAV_PARAM_TYPE: apm::MavParamType = apm::MavParamType::MAV_PARAM_TYPE_UINT8;
}

impl MavParam for u16 {
    const MAV_PARAM_TYPE: apm::MavParamType = apm::MavParamType::MAV_PARAM_TYPE_UINT16;
}

impl MavParam for u32 {
    const MAV_PARAM_TYPE: apm::MavParamType = apm::MavParamType::MAV_PARAM_TYPE_UINT32;
}

impl MavParam for u64 {
    const MAV_PARAM_TYPE: apm::MavParamType = apm::MavParamType::MAV_PARAM_TYPE_UINT64;
}

impl MavParam for i8 {
    const MAV_PARAM_TYPE: apm::MavParamType = apm::MavParamType::MAV_PARAM_TYPE_INT8;
}

impl MavParam for i16 {
    const MAV_PARAM_TYPE: apm::MavParamType = apm::MavParamType::MAV_PARAM_TYPE_INT16;
}

impl MavParam for i32 {
    const MAV_PARAM_TYPE: apm::MavParamType = apm::MavParamType::MAV_PARAM_TYPE_INT32;
}

impl MavParam for i64 {
    const MAV_PARAM_TYPE: apm::MavParamType = apm::MavParamType::MAV_PARAM_TYPE_INT64;
}
