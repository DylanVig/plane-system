use anyhow::Context;
use log::*;
use num_traits::{FromPrimitive, ToPrimitive};
use ptp::{ObjectFormatCode, ObjectHandle, PtpRead, StandardCommandCode, StorageId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;
use std::{collections::HashSet, fmt::Debug, time::Duration};

/// Sony's USB vendor ID
const SONY_USB_VID: u16 = 0x054C;
/// Sony R10C camera's product ID
const SONY_USB_R10C_PID: u16 = 0x0A79;
/// Sony R10C camera's product ID when it's powered off and charging
const SONY_USB_R10C_PID_CHARGING: u16 = 0x0994;

#[repr(u16)]
#[derive(ToPrimitive, FromPrimitive, Copy, Clone, Eq, PartialEq, Debug)]
pub enum CommandCode {
    SdioConnect = 0x96FE,
    SdioGetExtDeviceInfo = 0x96FD,
    SdioSetExtDevicePropValue = 0x96FA,
    SdioControlDevice = 0x96F8,
    SdioGetAllExtDevicePropInfo = 0x96F6,
    SdioSendUpdateFile = 0x96F5,
    SdioGetExtLensInfo = 0x96F4,
    SdioExtDeviceDeleteObject = 0x96F1,
}

impl Into<ptp::CommandCode> for CommandCode {
    fn into(self) -> ptp::CommandCode {
        ptp::CommandCode::Other(self.to_u16().unwrap())
    }
}

#[repr(u16)]
#[derive(ToPrimitive, FromPrimitive, Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum PropertyCode {
    AELock = 0xD6E8,
    AspectRatio = 0xD6B3,
    BatteryLevel = 0xD6F1,
    BatteryRemain = 0xD6E7,
    BiaxialAB = 0xD6E3,
    BiaxialGM = 0xD6EF,
    CaptureCount = 0xD633,
    Caution = 0xD6BA,
    ColorTemperature = 0xD6F0,
    Compression = 0xD6B9,
    DateTime = 0xD6B1,
    DriveMode = 0xD6B0,
    ExposureCompensation = 0xD6C3,
    ExposureMode = 0xD6CC,
    FNumber = 0xD6C5,
    FocusIndication = 0xD6EC,
    FocusMagnificationLevel = 0xD6A7,
    FocusMagnificationPosition = 0xD6A8,
    FocusMagnificationState = 0xD6A6,
    FocusMode = 0xD6CB,
    ImageSize = 0xD6B7,
    IntervalStillRecordingState = 0xD632,
    IntervalTime = 0xD631,
    ISO = 0xD6F2,
    LensStatus = 0xD6A9,
    LensUpdateState = 0xD624,
    LiveViewResolution = 0xD6AA,
    LiveViewStatus = 0xD6DE,
    LocationInfo = 0xD6C0,
    MediaFormatState = 0xD6C7,
    MovieFormat = 0xD6BD,
    MovieQuality = 0xD6BC,
    MovieRecording = 0xD6CD,
    MovieSteady = 0xD6D1,
    NotifyFocus = 0xD6AF,
    OperatingMode = 0xD6E2,
    SaveMedia = 0xD6CF,
    ShootingFileInfo = 0xD6C6,
    ShutterSpeed = 0xD6EA,
    StillSteadyMode = 0xD6D0,
    StorageInfo = 0xD6BB,
    WhiteBalance = 0xD6B8,
    WhiteBalanceInit = 0xD6DF,
    ZoomInfo = 0xD6BF,
    ZoomMagnificationInfo = 0xD63A,
    ZoomAbsolutePosition = 0xD6BE,
    Zoom = 0xD6C9,
}

#[repr(u16)]
#[derive(ToPrimitive, FromPrimitive, Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ControlCode {
    AELock = 0xD61E,
    AFLock = 0xD63B,
    CameraSettingReset = 0xD6D9,
    ExposureCompensation = 0xD6C3,
    FNumber = 0xD6C5,
    FocusFarForContinuous = 0xD6A4,
    FocusFarForOneShot = 0xD6A2,
    FocusMagnification = 0xD6A5,
    FocusNearForContinuous = 0xD6A3,
    FocusNearForOneShot = 0xD6A1,
    IntervalStillRecording = 0xD630,
    ISO = 0xD6F2,
    MediaFormat = 0xD61C,
    MovieRecording = 0xD60F,
    PowerOff = 0xD637,
    RequestForUpdate = 0xD612,
    RequestForUpdateForLens = 0xD625,
    S1Button = 0xD61D,
    S2Button = 0xD617,
    ShutterSpeed = 0xD6EA,
    SystemInit = 0xD6DA,
    ZoomControlAbsolute = 0xD60E,
    ZoomControlTele = 0xD63C,
    ZoomControlTeleOneShot = 0xD614,
    ZoomControlWide = 0xD63E,
    ZoomControlWideOneShot = 0xD613,
}

#[repr(u8)]
#[derive(
    ToPrimitive, FromPrimitive, Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize,
)]
pub enum OperatingMode {
    Standby = 0x01,
    StillRec,
    MovieRec,
    ContentsTransfer,
}

struct CameraState {
    version: u16,
}

pub struct CameraInterface {
    camera: ptp::Camera<rusb::GlobalContext>,
}

impl CameraInterface {
    pub fn timeout(&self) -> Option<Duration> {
        Some(Duration::from_secs(5))
    }

    pub fn new() -> anyhow::Result<Self> {
        let handle = rusb::open_device_with_vid_pid(SONY_USB_VID, SONY_USB_R10C_PID)
            .or_else(|| rusb::open_device_with_vid_pid(SONY_USB_VID, SONY_USB_R10C_PID_CHARGING))
            .context("could not open Sony R10C usb device")?;

        Ok(CameraInterface {
            camera: ptp::Camera::new(handle).context("could not initialize Sony R10C")?,
        })
    }

    pub fn connect(&mut self) -> anyhow::Result<()> {
        self.camera.open_session(self.timeout())?;

        let key_code = 0x0000DA01;

        // send SDIO_CONNECT twice, once with phase code 1, and then again with
        // phase code 2

        trace!("sending SDIO_Connect phase 1");

        self.camera.command(
            CommandCode::SdioConnect.into(),
            &[1, key_code, key_code],
            None,
            self.timeout(),
        )?;

        trace!("sending SDIO_Connect phase 2");

        self.camera.command(
            CommandCode::SdioConnect.into(),
            &[2, key_code, key_code],
            None,
            self.timeout(),
        )?;

        trace!("sending SDIO_GetExtDeviceInfo until success");

        let mut retries = 0;

        let state = loop {
            // call getextdeviceinfo with initiatorversion = 0x00C8

            let initiation_result = self.camera.command(
                CommandCode::SdioGetExtDeviceInfo.into(),
                &[0x00C8],
                None,
                self.timeout(),
            );

            match initiation_result {
                Ok(ext_device_info) => {
                    // Vec<u8> is not Read, but Cursor is
                    let mut ext_device_info = Cursor::new(ext_device_info);

                    let sdi_ext_version = PtpRead::read_ptp_u16(&mut ext_device_info)?;
                    let sdi_device_props = PtpRead::read_ptp_u16_vec(&mut ext_device_info)?;
                    let sdi_device_props = sdi_device_props
                        .into_iter()
                        .filter_map(<PropertyCode as FromPrimitive>::from_u16)
                        .collect::<HashSet<_>>();

                    let sdi_device_controls = PtpRead::read_ptp_u16_vec(&mut ext_device_info)?;
                    let sdi_device_controls = sdi_device_controls
                        .into_iter()
                        .filter_map(<ControlCode as FromPrimitive>::from_u16)
                        .collect::<HashSet<_>>();

                    trace!("got device props: {:?}", sdi_device_props);
                    trace!("got device controls: {:?}", sdi_device_controls);

                    break Ok(CameraState {
                        version: sdi_ext_version,
                    });
                }
                Err(err) => {
                    if retries < 1000 {
                        retries += 1;
                        continue;
                    } else {
                        break Err(err);
                    }
                }
            }
        }?;

        trace!("got extension version 0x{:04X}", state.version);

        trace!("sending SDIO_Connect phase 3");

        self.camera.command(
            CommandCode::SdioConnect.into(),
            &[3, key_code, key_code],
            None,
            self.timeout(),
        )?;

        trace!("connection complete");

        Ok(())
    }

    pub fn disconnect(&mut self) -> anyhow::Result<()> {
        self.camera.close_session(self.timeout())?;

        Ok(())
    }

    pub fn reset(&mut self) -> anyhow::Result<()> {
        self.camera.reset()?;

        Ok(())
    }

    pub fn query(&mut self) -> anyhow::Result<HashMap<PropertyCode, ptp::PropInfo>> {
        let timeout = self.timeout();

        trace!("sending SDIO_GetAllExtDevicePropInfo");

        let result = self.camera.command(
            CommandCode::SdioGetAllExtDevicePropInfo.into(),
            &[],
            None,
            timeout,
        )?;

        let mut cursor = Cursor::new(result);

        let num_entries = cursor.read_ptp_u8()? as usize;

        trace!("reading {:?} entries", num_entries);

        let mut properties = HashMap::new();

        for _ in 0..num_entries {
            let current_prop = ptp::PropInfo::decode(&mut cursor)?;

            let current_prop_code = match PropertyCode::from_u16(current_prop.property_code) {
                Some(code) => code,
                None => {
                    trace!(
                        "ignoring invalid property with code {:#0x}: {:?}",
                        current_prop.property_code,
                        current_prop
                    );
                    continue;
                }
            };

            properties.insert(current_prop_code, current_prop);
        }

        Ok(properties)
    }

    /// Sets the value of a camera property. This should be followed by a call
    /// to update() and a check to make sure that the intended result was
    /// achieved.
    pub fn set(&mut self, code: PropertyCode, new_value: ptp::Data) -> anyhow::Result<()> {
        let buf = new_value.encode();

        trace!("sending SDIO_SetExtDevicePropValue");

        self.camera.command(
            CommandCode::SdioSetExtDevicePropValue.into(),
            &[code.to_u32().unwrap()],
            Some(buf.as_ref()),
            self.timeout(),
        )?;

        Ok(())
    }

    /// Executes a command on the camera. This should be followed by a call to
    /// update() and a check to make sure that the intended result was achieved.
    pub fn execute(&mut self, code: ControlCode, payload: ptp::Data) -> anyhow::Result<()> {
        let buf = payload.encode();

        trace!("sending SDIO_ControlDevice");

        self.camera.command(
            CommandCode::SdioControlDevice.into(),
            &[code.to_u32().unwrap()],
            Some(buf.as_ref()),
            self.timeout(),
        )?;

        Ok(())
    }

    /// Receives an event from the camera.
    pub fn recv(&mut self, timeout: Option<Duration>) -> anyhow::Result<Option<ptp::Event>> {
        let event = self.camera.event(timeout)?;
        if let Some(event) = &event {
            trace!("received event: {:?}", event);
        }
        Ok(event)
    }

    pub fn device_info(&mut self, timeout: Option<Duration>) -> anyhow::Result<ptp::DeviceInfo> {
        Ok(self.camera.get_device_info(timeout)?)
    }

    pub fn storage_ids(&mut self, timeout: Option<Duration>) -> anyhow::Result<Vec<StorageId>> {
        Ok(self.camera.get_storage_ids(timeout)?)
    }

    pub fn storage_info(
        &mut self,
        storage_id: StorageId,
        timeout: Option<Duration>,
    ) -> anyhow::Result<ptp::StorageInfo> {
        Ok(self.camera.get_storage_info(storage_id, timeout)?)
    }

    pub fn object_handles(
        &mut self,
        storage_id: StorageId,
        parent_id: Option<ObjectHandle>,
        timeout: Option<Duration>,
    ) -> anyhow::Result<Vec<ObjectHandle>> {
        Ok(self
            .camera
            .get_object_handles(storage_id, None, parent_id, timeout)?)
    }

    pub fn object_info(
        &mut self,
        object_id: ObjectHandle,
        timeout: Option<Duration>,
    ) -> anyhow::Result<ptp::ObjectInfo> {
        Ok(self.camera.get_object_info(object_id, timeout)?)
    }

    pub fn object_data(
        &mut self,
        object_id: ObjectHandle,
        timeout: Option<Duration>,
    ) -> anyhow::Result<Vec<u8>> {
        Ok(self.camera.get_object(object_id, timeout)?)
    }

    pub fn init_capture(
        &mut self,
        storage: StorageId,
        format: ObjectFormatCode,
        timeout: Option<Duration>,
    ) -> anyhow::Result<Vec<u8>> {
        Ok(self.camera.command(
            StandardCommandCode::InitiateCapture.into(),
            &[storage.into(), format.to_u32().unwrap()],
            None,
            timeout,
        )?)
    }
}
