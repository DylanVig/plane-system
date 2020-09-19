use super::mavlink_async::{read_v2_msg, write_v2_msg};
use mavlink::{self, ardupilotmega as apm, MavHeader};
use smol::{io::AsyncRead, io::AsyncWrite};
use std::{sync::atomic::AtomicU8, sync::atomic::Ordering, fmt::Debug};

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

pub struct PixhawkInterface<R: AsyncRead + Unpin + Send, W: AsyncWrite + Unpin + Send> {
    reader: R,
    writer: W,
    sequence: AtomicU8,
}

impl<R: AsyncRead + Unpin + Send, W: AsyncWrite + Unpin + Send> PixhawkInterface<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        PixhawkInterface {
            reader,
            writer,
            sequence: AtomicU8::new(0),
        }
    }

    pub async fn send(&mut self, message: apm::MavMessage) -> anyhow::Result<()> {
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);

        let header = MavHeader {
            sequence,
            system_id: 0,
            component_id: 0,
        };

        write_v2_msg(&mut self.writer, header, &message).await?;

        Ok(())
    }

    /// Starts a task that will run the Pixhawk.
    pub async fn recv(&mut self) -> anyhow::Result<apm::MavMessage> {
        let (_, message) = read_v2_msg(&mut self.reader).await?;

        debug!("received message: {:?}", message);

        Ok(message)
    }
}
