use futures::{Sink, SinkExt, Stream, StreamExt};
use num_traits::FromPrimitive;
use simplebgc::*;
use std::io::{Read, Write};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_serial::{Serial, SerialPortSettings};
use tokio_util::codec::{Decoder, Encoder, Framed};

const SBGC_VID: u16 = 0x10C4;
const SBGC_PID: u16 = 0xEA60;
const FTDI_VID: u16 = 0x0403;
const FTDI_PID: u16 = 0x6001;

pub struct GimbalInterface {
    // TODO second type arg can be a dyn trait over both types
    inner: Framed<Serial, V1Codec>,
}

impl GimbalInterface {
    pub fn new() -> anyhow::Result<Self> {
        if let Some(device_path) = Self::find_usb_device_path()? {
            let settings = SerialPortSettings::default();
            let port = Serial::from_path(device_path, &settings)?;

            return Ok(Self {
                inner: V1Codec.framed(port),
            });
        } else {
            return Err(anyhow!("SimpleBGC usb device not found"));
        }
    }

    fn find_usb_device_path() -> anyhow::Result<Option<String>> {
        #[cfg(feature = "udev")]
        {
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

        Ok(None)
    }

    async fn send_command(&mut self, cmd: OutgoingCommand) -> anyhow::Result<()> {
        self.inner.send(cmd).await?;
        Ok(())
    }

    async fn get_response(&mut self) -> anyhow::Result<Option<IncomingCommand>> {
        if let Some(cmd_result) = self.inner.next().await {
            let cmd = cmd_result?;
            return Ok(Some(cmd));
        }
        Ok(None)
    }

    pub async fn control_angles(&mut self, mut roll: f64, mut pitch: f64) -> anyhow::Result<()> {
        info!("Got request for {}, {}", roll, pitch);
        if roll.abs() > 50.0 || pitch.abs() > 50.0 {
            roll = 0.0;
            pitch = 0.0;
        }

        let factor: f64 = (2 ^ 14) as f64 / 360.0;

        let command = OutgoingCommand::Control(ControlData {
            mode: ControlFormat::Legacy(AxisControlState::from_u8(0x02).unwrap()),
            axes: RollPitchYaw {
                roll: AxisControlParams {
                    /// unit conversion: SBGC units are 360 / 2^14 degrees
                    angle: (roll * factor) as i16,
                    speed: 1200,
                },
                pitch: AxisControlParams {
                    /// unit conversion: SBGC units are 360 / 2^14 degrees
                    angle: (pitch * factor) as i16,
                    speed: 2400,
                },
                yaw: AxisControlParams { angle: 0, speed: 0 },
            },
        });

        self.send_command(command).await?;

        // TODO: we need to implement CMD_CONFIRM in the simplebgc-rs crate
        // let response = self.get_response()?;

        Ok(())
    }
}
