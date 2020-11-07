use anyhow::Context;
use cxx::UniquePtr;
use num_traits::ToPrimitive;
use std::{
    error::Error,
    fmt::{Debug, Display},
    time::Duration,
};

#[cxx::bridge]
mod ffi {

    extern "C" {
        include!("socc_types.h");
        include!("socc_examples_fixture.h");

        type socc_examples_fixture;
        type socc_ptp;
        type socc_error;
        type SDIDevicePropInfoDataset;

        fn make_ptp() -> UniquePtr<socc_ptp>;
        fn make_fixture(ptp: &mut socc_ptp) -> UniquePtr<socc_examples_fixture>;

        fn connect(self: &mut socc_examples_fixture) -> i32;
        fn disconnect(self: &mut socc_examples_fixture) -> i32;

        #[cxx_name = "OpenSession"]
        fn open_session(self: &mut socc_examples_fixture, session_id: u32) -> i32;
        #[cxx_name = "CloseSession"]
        fn close_session(self: &mut socc_examples_fixture) -> ();

        #[cxx_name = "SDIO_Connect"]
        fn sdio_connect(
            self: &mut socc_examples_fixture,
            phase_type: u32,
            keycode1: u32,
            keycode2: u32,
        ) -> i32;

        #[cxx_name = "SDIO_SetExtDevicePropValue"]
        fn sdio_set_ext_device_prop_value_u8(
            self: &mut socc_examples_fixture,
            code: u16,
            data: u8,
        ) -> i32;
        #[cxx_name = "SDIO_SetExtDevicePropValue"]
        fn sdio_set_ext_device_prop_value_u16(
            self: &mut socc_examples_fixture,
            code: u16,
            data: u16,
        ) -> i32;
        #[cxx_name = "SDIO_SetExtDevicePropValue_str"]
        fn sdio_set_ext_device_prop_value_str(
            self: &mut socc_examples_fixture,
            code: u16,
            data: &str,
        ) -> i32;

        #[cxx_name = "SDIO_ControlDevice"]
        fn sdio_control_device_u16(self: &mut socc_examples_fixture, code: u16, value: u16) -> i32;

        #[cxx_name = "wait_for_InitiatorVersion"]
        fn wait_for_initiator_version(
            self: &mut socc_examples_fixture,
            expect: u16,
            retry_count: i32,
        ) -> i32;

        /// Actually returns `*const SDIDevicePropInfoDataset` but raw pointers
        /// are not supported by cxx yet
        #[cxx_name = "wait_for_IsEnable_usize"]
        fn wait_for_enable(
            self: &mut socc_examples_fixture,
            code: u16,
            expect: u16,
            retry_count: i32,
        ) -> usize;

        #[cxx_name = "wait_for_CurrentValue"]
        fn wait_for_current_value(
            self: &mut socc_examples_fixture,
            code: u16,
            expect: u16,
            retry_count: i32,
        ) -> i32;
    }

    extern "Rust" {}
}

// TODO: verify that the C++ code isn't doing anything thread-unsafe 🤨
unsafe impl Send for ffi::socc_examples_fixture {}
unsafe impl Send for ffi::socc_ptp {}

#[derive(FromPrimitive, Debug, Copy, Clone)]
#[repr(i32)]
pub enum CameraError {
    NotSupported = -1,
    InvalidParamter = -2,

    UsbInit = -101,
    UsbDeviceNotFound = -102,
    UsbOpen = -103,
    UsbTimeout = -104,
    UsbEndpointHalted = -105,
    UsbOverflow = -106,
    UsbDisconnected = -107,
    UsbOther = -108,

    ThreadInit = -201,
    ThreadCreate = -202,

    PtpTransaction = -301,
}

impl Display for CameraError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for CameraError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

pub struct CameraInterface {
    _ptp: UniquePtr<ffi::socc_ptp>,
    fixture: UniquePtr<ffi::socc_examples_fixture>,
    connected: bool,
}

macro_rules! check_camera_result {
    ($result: expr) => {{
        use anyhow::Context;
        use num_traits::FromPrimitive;

        match $result {
            0 => {}
            err => {
                if let Some(err) = <CameraError as FromPrimitive>::from_i32(err) {
                    return Err(err).context(stringify!($result));
                }

                return Err(anyhow!("Unknown camera error {:?}", err));
            }
        }
    }};
}

impl CameraInterface {
    pub fn new() -> Self {
        let mut ptp = unsafe { ffi::make_ptp() };
        let fixture = unsafe { ffi::make_fixture(&mut ptp) };

        CameraInterface {
            _ptp: ptp,
            fixture,
            connected: false,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn connect(&mut self) -> anyhow::Result<()> {
        check_camera_result!(self.fixture.connect());

        check_camera_result!(self.fixture.open_session(1));

        check_camera_result!(self.fixture.sdio_connect(0x000001, 0x0000DA01, 0x0000DA01));
        check_camera_result!(self.fixture.sdio_connect(0x000002, 0x0000DA01, 0x0000DA01));
        check_camera_result!(self.fixture.wait_for_initiator_version(0x00C8, 1000));
        check_camera_result!(self.fixture.sdio_connect(0x000003, 0x0000DA01, 0x0000DA01));

        self.fixture.wait_for_enable(0xD6B1, 0x01, 1000);
        check_camera_result!(self
            .fixture
            .sdio_set_ext_device_prop_value_str(0xD6B1, "20150801T150000+0900"));

        self.fixture.wait_for_enable(0xD6E2, 0x01, 1000);
        self.fixture.sdio_set_ext_device_prop_value_u8(0xD6E2, 0x02);
        self.fixture.wait_for_current_value(0xD6E2, 0x02, 1000);

        self.fixture
            .sdio_set_ext_device_prop_value_u16(0xD6CF, 0x0001);
        self.fixture.wait_for_current_value(0xD6CF, 0x0001, 1000);

        self.fixture.wait_for_current_value(0xD6DE, 0x01, 1000);
        self.connected = true;

        Ok(())
    }

    pub fn take_photo(&mut self) -> anyhow::Result<()> {
        /* SDIO_ControlDevice, s1 down */
        check_camera_result!(self.fixture.sdio_control_device_u16(0xD61D, 0x0002));

        /* SDIO_ControlDevice, s2 down */
        check_camera_result!(self.fixture.sdio_control_device_u16(0xD617, 0x0002));
        std::thread::sleep(Duration::from_millis(100));

        /* SDIO_ControlDevice, s2 up */
        check_camera_result!(self.fixture.sdio_control_device_u16(0xD617, 0x0001));
        std::thread::sleep(Duration::from_millis(100));

        /* SDIO_ControlDevice, s1 down */
        check_camera_result!(self.fixture.sdio_control_device_u16(0xD61D, 0x0001));

        Ok(())
    }

    pub fn disconnect(&mut self) -> anyhow::Result<()> {
        self.fixture.close_session();

        check_camera_result!(self.fixture.disconnect());
        self.connected = false;

        Ok(())
    }
}

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
            .context("could not find Sony R10C usb device")?;

        Ok(CameraInterface2 {
            camera: ptp::PtpCamera::new(handle).context("could not initialize Sony R10C")?,
        })
    }

    pub fn connect(&mut self) -> anyhow::Result<()> {
        use ptp::PtpRead;
        use std::io::{Cursor, Read};

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
