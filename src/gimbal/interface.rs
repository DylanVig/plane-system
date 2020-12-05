use num_traits::FromPrimitive;
use simplebgc::*;
use std::io::{Read, Write};
use std::time::Duration;
use tokio::time::*;

const SBGC_VID: u16 = 0x10C4;
const SBGC_PID: u16 = 0xEA60;

pub struct GimbalInterface {
    port: serialport::TTYPort,
}

impl GimbalInterface {
    pub fn new() -> anyhow::Result<Self> {
        if let Some(device_name) = Self::find_usb_device_name()? {
            let port = serialport::new(device_name, 115_200)
                .timeout(Duration::from_millis(10))
                .open_native()?;

            return Ok(Self { port });
        } else {
            return Err(anyhow!("SimpleBGC usb device not found"));
        }
    }

    fn find_usb_device_name() -> anyhow::Result<Option<String>> {
        let ports = serialport::available_ports()?;
        for port in ports {
            match port.port_type {
                serialport::SerialPortType::UsbPort(info) => {
                    if info.vid == SBGC_VID && info.pid == SBGC_PID {
                        return Ok(Some(port.port_name));
                    }
                }
                _ => continue,
            }
        }
        Ok(None)
    }

    fn send_command(&mut self, cmd: OutgoingCommand) -> anyhow::Result<()> {
        let bytes = cmd.to_v1_bytes();
        self.port.write(&bytes[..])?;
        Ok(())
    }

    fn get_response(&mut self) -> anyhow::Result<IncomingCommand> {
        let mut buf: Vec<u8> = vec![0; 4096];
        let marker = self.port.read(buf.as_mut_slice())?;
        let (cmd, _) = IncomingCommand::from_v1_bytes(&buf[..marker])?;
        Ok(cmd)
    }

    pub fn control_angles(&mut self, mut roll: f64, mut pitch: f64) -> anyhow::Result<()> {
        info!("Got request for {}, {}", roll, pitch);
        if roll.abs() > 50.0 || pitch.abs() > 50.0 {
            roll = 0.0;
            pitch = 0.0;
        }
        let base: i32 = 2;
        let command = OutgoingCommand::Control(ControlData {
            mode: ControlFormat::Legacy(AxisControlState::from_u8(0x02).unwrap()),
            axes: RollPitchYaw {
                roll: AxisControlParams {
                    /// unit conversion: SBGC units are 360 / 2^14 degrees
                    angle: ((roll * base.pow(14) as f64) / 360.0) as i16,
                    speed: 1200,
                },
                pitch: AxisControlParams {
                    /// unit conversion: SBGC units are 360 / 2^14 degrees
                    angle: ((pitch * base.pow(14) as f64) / 360.0) as i16,
                    speed: 2400,
                },
                yaw: AxisControlParams { angle: 0, speed: 0 },
            },
        });
        self.send_command(command)?;
        // TODO: we need to implement CMD_CONFIRM in the simplebgc-rs crate
        // let response = self.get_response()?;
        Ok(())
    }
}
