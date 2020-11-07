fn main() {
    // cxx_build::bridge("src/camera/interface.rs")
    //     .include("vendor/libsoccptp/include")
    //     .include("vendor/libsoccptp/ports")
    //     .file("vendor/libsoccptp/ports/ports_usb_impl.cpp")
    //     .file("vendor/libsoccptp/ports/ports_ptp_impl.cpp")
    //     .file("vendor/libsoccptp/sources/parser.cpp")
    //     .file("vendor/libsoccptp/sources/socc_ptp.cpp")
    //     .flag_if_supported("-std=c++14")
    //     .warnings(false)
    //     .compile("soccptp");

    // println!("cargo:rerun-if-changed=src/camera/interface.rs");
    // println!("cargo:rustc-link-lib=usb-1.0");
}
