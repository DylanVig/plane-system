use cxx::UniquePtr;
use std::{
    error::Error,
    fmt::{Debug, Display},
};

#[cxx::bridge]
mod ffi {
    extern "C" {
        include!("socc_types.h");
        include!("socc_examples_fixture.h");

        type socc_examples_fixture;
        type socc_ptp;
        type socc_error;

        fn make_ptp() -> UniquePtr<socc_ptp>;
        fn make_fixture(ptp: &mut socc_ptp) -> UniquePtr<socc_examples_fixture>;

        fn connect(self: &mut socc_examples_fixture) -> i32;
        fn disconnect(self: &mut socc_examples_fixture) -> i32;

        fn OpenSession(self: &mut socc_examples_fixture, session_id: u32) -> i32;
        fn CloseSession(self: &mut socc_examples_fixture) -> ();

        fn SDIO_Connect(
            self: &mut socc_examples_fixture,
            phase_type: u32,
            keycode1: u32,
            keycode2: u32,
        ) -> i32;
        fn SDIO_SetExtDevicePropValue_u8(
            self: &mut socc_examples_fixture,
            code: u16,
            data: u8,
        ) -> i32;
        fn SDIO_SetExtDevicePropValue_u16(
            self: &mut socc_examples_fixture,
            code: u16,
            data: u16,
        ) -> i32;
        fn SDIO_SetExtDevicePropValue_str(
            self: &mut socc_examples_fixture,
            code: u16,
            data: &str,
        ) -> i32;
        fn SDIO_ControlDevice_u16(
            self: &mut socc_examples_fixture,
            code: u16,
            value: u16,
        ) -> i32;

        fn wait_for_InitiatorVersion(
            self: &mut socc_examples_fixture,
            expect: u16,
            retry_count: i32,
        ) -> i32;
        /// Actually returns `*const SDIDevicePropInfoDataset` but raw pointers
        /// are not supported by cxx yet
        fn wait_for_IsEnable_casted(
            self: &mut socc_examples_fixture,
            code: u16,
            expect: u16,
            retry_count: i32,
        ) -> usize;
        fn wait_for_CurrentValue(
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

pub struct CameraClient {
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

impl CameraClient {
    pub fn new() -> Self {
        let mut ptp = unsafe { ffi::make_ptp() };
        let fixture = unsafe { ffi::make_fixture(&mut ptp) };

        CameraClient {
            _ptp: ptp,
            fixture,
            connected: false,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub async fn connect(&mut self) -> anyhow::Result<()> {
        smol::unblock(|| {
            check_camera_result!(self.fixture.connect());

            check_camera_result!(self.fixture.OpenSession(1));

            check_camera_result!(self.fixture.SDIO_Connect(0x000001, 0x0000DA01, 0x0000DA01));
            check_camera_result!(self.fixture.SDIO_Connect(0x000002, 0x0000DA01, 0x0000DA01));
            check_camera_result!(self.fixture.wait_for_InitiatorVersion(0x00C8, 1000));
            check_camera_result!(self.fixture.SDIO_Connect(0x000003, 0x0000DA01, 0x0000DA01));

            self.fixture.wait_for_IsEnable_casted(0xD6B1, 0x01, 1000);
            check_camera_result!(self
                .fixture
                .SDIO_SetExtDevicePropValue_str(0xD6B1, "20150801T150000+0900"));

            self.fixture.wait_for_IsEnable_casted(0xD6E2, 0x01, 1000);
            self.fixture.SDIO_SetExtDevicePropValue_u8(0xD6E2, 0x02);
            self.fixture.wait_for_CurrentValue(0xD6E2, 0x02, 1000);

            self.fixture.SDIO_SetExtDevicePropValue_u16(0xD6CF, 0x0001);
            self.fixture.wait_for_CurrentValue(0xD6CF, 0x0001, 1000);

            self.fixture.wait_for_CurrentValue(0xD6DE, 0x01, 1000);
            self.connected = true;

            Ok(())
        })
        .await?
    }

    pub async fn take_photo(&mut self) -> anyhow::Result<()> {
        /* SDIO_ControlDevice, s1 down */
        check_camera_result!(self.fixture.SDIO_ControlDevice(0xD61D, (uint16_t)0x0002));

        /* SDIO_ControlDevice, s2 down */
        check_camera_result!(self.fixture.SDIO_ControlDevice(0xD617, (uint16_t)0x0002));
        check_camera_result!(self.fixture.milisleep(100));

        /* SDIO_ControlDevice, s2 up */
        check_camera_result!(self.fixture.SDIO_ControlDevice(0xD617, (uint16_t)0x0001));
        check_camera_result!(self.fixture.milisleep(100));

        /* SDIO_ControlDevice, s1 down */
        check_camera_result!(self.fixture.SDIO_ControlDevice(0xD61D, (uint16_t)0x0001));
    }

    pub fn disconnect(&mut self) {}
}
