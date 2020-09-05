fn main() {
    println!("Hello, world!");
}

#[cxx::bridge]
mod ffi {
    extern "C" {
        include!("socc_types.h");

        type socc_device_handle_info_t;
    }

    extern "Rust" {}
}
