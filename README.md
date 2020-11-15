# `@cuair/plane-system`

## how to compile

- Install [Rust 1.46.0 or higher](https://rustup.rs/)
- Install `libusb`
  - Arch/Manjaro: `pacman -S libusb`
  - Ubuntu/Debian: `apt install libusb-1.0-0-dev`
- `cargo build`

## how to cross-compile (fast)

- Install Rust
- Install Raspberry Pi toolchain: `rustup install aarch64-unknown-linux-gnu`
- Cross-compile: `cargo build --target=aarch64-unknown-linux-gnu`


## how to cross-compile (slow, but easy)
Try this if the fast method fails.

- Install Rust
- Install Rust Cross: `cargo install cross`
- Build Docker image: `docker build -t cuair/obc:0.2 .`
- Cross-compile: `cross build --target=aarch64-unknown-linux-gnu`

## how to run

- Start the SITL on the `new-plane-system` branch: `./run.sh -S` should work
- Start the plane server: `RUST_LOG=info cargo run`

## faq

>  Why don't I see any output?

You probably forgot to set the [`RUST_LOG`](https://docs.rs/env_logger/latest/env_logger/) environment variable

> Why isn't the pixhawk telemetry stream being parsed successfully?

Ensure that the pixhawk is reporting telemetry in MAVLink v1.
