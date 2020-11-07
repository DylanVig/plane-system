use anyhow::Context;
use num_traits::ToPrimitive;
use std::{
    fmt::Debug,
    time::Duration,
};

use ptp::PtpRead;
use std::io::Cursor;

/// Sony's USB vendor ID
const SONY_USB_VID: u16 = 0x054C;
/// Sony R10C camera's product ID
const SONY_USB_PID: u16 = 0x0A79;
/// Sony's PTP extension vendor ID
const SONY_PTP_VID: u16 = 0x0011;

#[repr(u16)]
#[derive(ToPrimitive, FromPrimitive, Copy, Clone, Eq, PartialEq, Debug)]
pub enum SonyCommandCode {
    SdioConnect = 0x96FE,
    SdioGetExtDeviceInfo = 0x96FD,
    SdioSetExtDevicePropValue = 0x96FA,
    SdioControlDevice = 0x96F8,
    SdioGetAllExtDevicePropInfo = 0x96F6,
    SdioSendUpdateFile = 0x96F5,
    SdioGetExtLensInfo = 0x96F4,
    SdioExtDeviceDeleteObject = 0x96F1,
}

impl Into<ptp::CommandCode> for SonyCommandCode {
    fn into(self) -> ptp::CommandCode {
        ptp::CommandCode::Other(self.to_u16().unwrap())
    }
}

pub struct CameraInterface2 {
    camera: ptp::PtpCamera<rusb::GlobalContext>,
}

impl CameraInterface2 {
    pub fn timeout(&self) -> Option<Duration> {
        Some(Duration::from_secs(5))
    }

    pub fn new() -> anyhow::Result<Self> {
        let handle = rusb::open_device_with_vid_pid(SONY_USB_VID, SONY_USB_PID)
            .context("could not open Sony R10C usb device")?;

        Ok(CameraInterface2 {
            camera: ptp::PtpCamera::new(handle).context("could not initialize Sony R10C")?,
        })
    }

    pub fn connect(&mut self) -> anyhow::Result<()> {
        self.camera.open_session(self.timeout())?;

        let key_code = 0x0000DA01;

        // send SDIO_CONNECT twice, once with phase code 1, and then again with
        // phase code 2

        trace!("sending SDIO_Connect phase 1");

        self.camera.command(
            SonyCommandCode::SdioConnect.into(),
            &[1, key_code, key_code],
            None,
            self.timeout(),
        )?;

        trace!("sending SDIO_Connect phase 2");

        self.camera.command(
            SonyCommandCode::SdioConnect.into(),
            &[2, key_code, key_code],
            None,
            self.timeout(),
        )?;

        trace!("sending SDIO_GetExtDeviceInfo until success");

        let mut retries = 0;

        let sdi_ext_version = loop {
            // call getextdeviceinfo with initiatorversion = 0x00C8

            let initiation_result = self.camera.command(
                SonyCommandCode::SdioGetExtDeviceInfo.into(),
                &[0x00C8],
                None,
                self.timeout(),
            );

            match initiation_result {
                Ok(ext_device_info) => {
                    // Vec<u8> is not Read, but Cursor is
                    let mut ext_device_info = Cursor::new(ext_device_info);

                    let sdi_ext_version = PtpRead::read_ptp_u16(&mut ext_device_info)?;

                    break Ok(sdi_ext_version);
                }
                Err(err) => {
                    if retries < 1000 {
                        continue;
                    } else {
                        break Err(err);
                    }
                }
            }
        }?;

        trace!("got extension version {:04x}", sdi_ext_version);

        trace!("sending SDIO_Connect phase 3");

        self.camera.command(
            SonyCommandCode::SdioConnect.into(),
            &[3, key_code, key_code],
            None,
            self.timeout(),
        )?;

        trace!("connection complete");

        Ok(())
    }
}
