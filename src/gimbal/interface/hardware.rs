use anyhow::Context;
use num_traits::FromPrimitive;
use simplebgc::*;
use std::io::{Read, Write};
use std::time::Duration;

use super::GimbalInterface;

const SBGC_VID: u16 = 0x10C4;
const SBGC_PID: u16 = 0xEA60;

pub struct HardwareGimbalInterface {
    port: serialport::TTYPort,
}

impl HardwareGimbalInterface {}

impl GimbalInterface for HardwareGimbalInterface {
    fn new() -> anyhow::Result<Self> {
        // find USB device name
        let device_name = serialport::available_ports()?
            .into_iter()
            .filter_map(|port| match port.port_type {
                serialport::SerialPortType::UsbPort(info) => {
                    if info.vid == SBGC_VID && info.pid == SBGC_PID {
                        Some(port.port_name)
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .next()
            .context("simplebgc usb device not found")?;

        let port = serialport::new(device_name, 115_200)
            .timeout(Duration::from_millis(10))
            .open_native()?;

        return Ok(Self { port });
    }

    fn send_command(&mut self, cmd: OutgoingCommand) -> anyhow::Result<()> {
        let bytes = cmd.to_v1_bytes();
        self.port.write(&bytes[..])?;
        Ok(())
    }

    fn recv_command(&mut self) -> anyhow::Result<Option<IncomingCommand>> {
        let mut buf: Vec<u8> = vec![0; 4096];
        let marker = self.port.read(buf.as_mut_slice())?;
        let (cmd, _) = IncomingCommand::from_v1_bytes(&buf[..marker])?;
        Ok(Some(cmd))
    }
}
