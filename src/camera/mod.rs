use cxx::UniquePtr;
use std::{error::Error, fmt::{Display, Debug}};

#[cxx::bridge]
mod ffi {
    #[repr(i32)]
    enum socc_error {
        SOCC_OK = 0,
        SOCC_ERROR_NOT_SUPPORT = -1,
        SOCC_ERROR_INVALID_PARAMETER = -2,

        SOCC_ERROR_USB_INIT = -101,
        SOCC_ERROR_USB_DEVICE_NOT_FOUND = -102,
        SOCC_ERROR_USB_OPEN = -103,
        SOCC_ERROR_USB_TIMEOUT = -104,
        SOCC_ERROR_USB_ENDPOINT_HALTED = -105,
        SOCC_ERROR_USB_OVERFLOW = -106,
        SOCC_ERROR_USB_DISCONNECTED = -107,
        SOCC_ERROR_USB_OTHER = -108,

        SOCC_ERROR_THREAD_INIT = -201,
        SOCC_ERROR_THREAD_CREATE = -202,

        SOCC_PTP_ERROR_TRANSACTION = -301,
    }

    extern "C" {
        include!("socc_types.h");
        include!("socc_examples_fixture.h");

        type socc_examples_fixture;
        type socc_ptp;
        type socc_error;

        fn make_ptp() -> UniquePtr<socc_ptp>;
        fn make_fixture(ptp: &mut socc_ptp) -> UniquePtr<socc_examples_fixture>;

        fn connect(self: &mut socc_examples_fixture) -> socc_error;
        fn disconnect(self: &mut socc_examples_fixture) -> socc_error;

        fn OpenSession(self: &mut socc_examples_fixture, session_id: u32) -> ();
        fn CloseSession(self: &mut socc_examples_fixture) -> ();

        fn SDIO_Connect(
            self: &mut socc_examples_fixture,
            phase_type: u32,
            keycode1: u32,
            keycode2: u32,
        ) -> ();
        fn SDIO_SetExtDevicePropValue_u8(
            self: &mut socc_examples_fixture,
            code: u16,
            data: u8,
        ) -> ();
        fn SDIO_SetExtDevicePropValue_u16(
            self: &mut socc_examples_fixture,
            code: u16,
            data: u16,
        ) -> ();
        fn SDIO_SetExtDevicePropValue_str(
            self: &mut socc_examples_fixture,
            code: u16,
            data: &str,
        ) -> ();

        fn wait_for_InitiatorVersion(
            self: &mut socc_examples_fixture,
            expect: u16,
            retry_count: i32,
        ) -> ();
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
        ) -> ();
    }

    extern "Rust" {}
}

// TODO: verify that the C++ code isn't doing anything thread-unsafe 🤨
unsafe impl Send for ffi::socc_examples_fixture {}
unsafe impl Send for ffi::socc_ptp {}

impl Debug for ffi::socc_error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::SOCC_OK => write!(f, "OK"),
            Self::SOCC_ERROR_NOT_SUPPORT => write!(f, "ERROR_NOT_SUPPORT"),
            Self::SOCC_ERROR_INVALID_PARAMETER => write!(f, "ERROR_INVALID_PARAMETER"),
            Self::SOCC_ERROR_USB_INIT => write!(f, "ERROR_USB_INIT"),
            Self::SOCC_ERROR_USB_DEVICE_NOT_FOUND => write!(f, "ERROR_USB_DEVICE_NOT_FOUND"),
            Self::SOCC_ERROR_USB_OPEN => write!(f, "ERROR_USB_OPEN"),
            Self::SOCC_ERROR_USB_TIMEOUT => write!(f, "ERROR_USB_TIMEOUT"),
            Self::SOCC_ERROR_USB_ENDPOINT_HALTED => write!(f, "ERROR_USB_ENDPOINT_HALTED"),
            Self::SOCC_ERROR_USB_OVERFLOW => write!(f, "ERROR_USB_OVERFLOW"),
            Self::SOCC_ERROR_USB_DISCONNECTED => write!(f, "ERROR_USB_DISCONNECTED"),
            Self::SOCC_ERROR_USB_OTHER => write!(f, "ERROR_USB_OTHER"),
            Self::SOCC_ERROR_THREAD_INIT => write!(f, "ERROR_THREAD_INIT"),
            Self::SOCC_ERROR_THREAD_CREATE => write!(f, "ERROR_THREAD_CREATE"),
            Self::SOCC_PTP_ERROR_TRANSACTION => write!(f, "PTP_ERROR_TRANSACTION"),
            other => write!(f, "({:04x})", other.repr),
        }
    }
}

impl Display for ffi::socc_error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "socc_error::{:?}", self)
    }
}

impl Error for ffi::socc_error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

pub struct Camera {
    _ptp: UniquePtr<ffi::socc_ptp>,
    fixture: UniquePtr<ffi::socc_examples_fixture>,
    connected: bool,
}

impl Camera {
    pub fn new() -> Self {
        let mut ptp = ffi::make_ptp();
        let fixture = ffi::make_fixture(&mut ptp);
        
        Camera {
            _ptp: ptp,
            fixture,
            connected: false,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn connect(&mut self) -> anyhow::Result<()> {
        match self.fixture.connect() {
            ffi::socc_error::SOCC_OK => {}
            err => return Err(err.into()),
        };

        self.fixture.OpenSession(1);

        self.fixture.SDIO_Connect(0x000001, 0x0000DA01, 0x0000DA01);
        self.fixture.SDIO_Connect(0x000002, 0x0000DA01, 0x0000DA01);
        self.fixture.wait_for_InitiatorVersion(0x00C8, 1000);
        self.fixture.SDIO_Connect(0x000003, 0x0000DA01, 0x0000DA01);

        self.fixture.wait_for_IsEnable_casted(0xD6B1, 0x01, 1000);
        self.fixture
            .SDIO_SetExtDevicePropValue_str(0xD6B1, "20150801T150000+0900");

        self.fixture.wait_for_IsEnable_casted(0xD6E2, 0x01, 1000);
        self.fixture.SDIO_SetExtDevicePropValue_u8(0xD6E2, 0x02);
        self.fixture.wait_for_CurrentValue(0xD6E2, 0x02, 1000);

        self.fixture.SDIO_SetExtDevicePropValue_u16(0xD6CF, 0x0001);
        self.fixture.wait_for_CurrentValue(0xD6CF, 0x0001, 1000);

        self.fixture.wait_for_CurrentValue(0xD6DE, 0x01, 1000);
        self.connected = true;

        Ok(())
    }

    pub fn disconnect(&mut self) {

    }
}
