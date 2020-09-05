use cxx::UniquePtr;

fn main() {
    println!("Hello, world!");
    let fix = unsafe { ffi::make_fixture() };
}

#[cxx::bridge]
mod ffi {

    extern "C" {
        include!("socc_types.h");
        include!("socc_examples_fixture.h");

        type socc_examples_fixture;

        fn make_fixture() -> UniquePtr<socc_examples_fixture>;
    }

    extern "Rust" {}
}
