use anyhow::Context;
use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use simplebgc::*;
use std::path::Path;
use tokio_serial::{SerialPortBuilder, SerialStream};
use tokio_util::codec::{Decoder, Framed};
use tracing::log::*;

use super::SimpleBgcGimbalInterface;

pub struct HardwareGimbalInterface {
    inner: Framed<SerialStream, V1Codec>,
}

impl HardwareGimbalInterface {
    pub fn with_path<P: AsRef<str>>(device_path: P) -> anyhow::Result<Self> {
        let port = SerialStream::open(&tokio_serial::new(device_path.as_ref(), 115_200))?;

        return Ok(Self {
            inner: V1Codec.framed(port),
        });
    }

    fn find_usb_device_path() -> anyhow::Result<Option<String>> {
        #[cfg(feature = "udev")]
        {
            const SBGC_VID: u16 = 0x10C4;
            const SBGC_PID: u16 = 0xEA60;
            const FTDI_VID: u16 = 0x0403;
            const FTDI_PID: u16 = 0x6001;

            let ports = serialport::available_ports()?;
            info!("{:?}", ports);
            for port in ports {
                match port.port_type {
                    serialport::SerialPortType::UsbPort(info) => {
                        if (info.vid == SBGC_VID && info.pid == SBGC_PID)
                            || (info.vid == FTDI_VID && info.pid == FTDI_PID)
                        {
                            return Ok(Some(port.port_name));
                        }
                    }
                    _ => continue,
                }
            }
        }

        #[cfg(not(feature = "udev"))]
        {
            warn!("USB serial devices cannot be automatically detected because this executable was not compiled with udev enabled");
        }

        Ok(None)
    }
}

impl HardwareGimbalInterface {
    pub fn new() -> anyhow::Result<Self> {
        Self::with_path(
            Self::find_usb_device_path()?.context("could not find SimpleBGC USB device")?,
        )
    }
}

#[async_trait]
impl SimpleBgcGimbalInterface for HardwareGimbalInterface {
    async fn send_command(&mut self, cmd: OutgoingCommand) -> anyhow::Result<()> {
        self.inner.send(cmd).await?;
        Ok(())
    }

    async fn recv_command(&mut self) -> anyhow::Result<Option<IncomingCommand>> {
        if let Some(cmd_result) = self.inner.next().await {
            let cmd = cmd_result?;
            return Ok(Some(cmd));
        }
        Ok(None)
    }
}
