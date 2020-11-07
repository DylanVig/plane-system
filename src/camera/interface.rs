use cxx::UniquePtr;
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
        fn make_fixture(ptp: &mut socc_ptp) -> i32;

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
