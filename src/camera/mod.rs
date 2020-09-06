use cxx::UniquePtr;

#[cxx::bridge]
mod ffi {
    extern "C" {
        include!("socc_types.h");
        include!("socc_examples_fixture.h");

        type socc_examples_fixture;

        fn make_fixture() -> UniquePtr<socc_examples_fixture>;

        fn connect(self: &mut socc_examples_fixture) -> ();
        fn disconnect(self: &mut socc_examples_fixture) -> ();

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

pub struct Camera {
    fixture: UniquePtr<ffi::socc_examples_fixture>,
    connected: bool,
    busy: bool,
}

impl Camera {
    pub fn new() -> Self {
        Camera {
            fixture: unsafe { ffi::make_fixture() },
            connected: false,
            busy: false,
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn connect(&mut self) {
        self.fixture.connect();
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
    }
}
