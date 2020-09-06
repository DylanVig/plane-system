use cxx::UniquePtr;
use rusb;

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
}
