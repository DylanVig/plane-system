# `@cuair/plane-system`

## how to compile

The plane system was designed to run on Linux. It might run on macOS, but it
also might not. Don't even think about running it on Windows.

- Install [Rust 1.46.0 or higher](https://rustup.rs/)
  - The minimum supported Rust version is probably lower than this, but this is
    the version we started developing on
- Install `libusb`
  - Arch/Manjaro: `pacman -S libusb`
  - Ubuntu/Debian: `apt install libusb-1.0-0-dev`
  - macOS: `brew install libusb`
- `cargo build`

## how to cross-compile

- Log into our Docker registry (DM Ibiyemi for password):

  ```bash
  docker login dr.cuair.org
  ```
- Run `docker/user/build.sh <target>` where `<target>` is one of our on-board
  computer configurations. At the time of writing, there are two:
  - `rpi3-raspbian-v1`
  - `rpi4-ubuntu-v1`

## how to run

- If you want to test with the SITL:
  - Start the SITL on the `new-plane-system` branch: `./run.sh -S -O 172.18.0.1` in the MAVProxy repo
    - The `-O 172.18.0.1` instructs MAVProxy to forward MAVLink packets to
      `172.18.0.1`, which should be the IP address of your Docker network's
      gateway. This way they will show up at port 14551 on the host machine.
  - Make sure you have the following in `plane-system.json`:
    ```json
    "pixhawk": {
      "address": "0.0.0.0:14551",
      "mavlink": { "type": "V2" }
    }
    ```
- If you want to test with the camera:
  - Ensure that the camera is plugged in and the current user has permissions to
    control the camera. You can either run as root (not ideal) or create a
    `udev` rule to give your user access to the camera.
  - Make sure you have `"camera": true` in `plane-system.json`
- If you want to test with the gimbal:
  - Ensure that the gimbal is plugged in.
  - Make sure you have `"gimbal": true` in `plane-system.json`
- Start the plane server:
  - In development mode, w/ source code available: `RUST_LOG=plane_system=debug cargo run`
  - In production, w/ just the binary: `RUST_LOG=plane_system=info ./plane-system --config=plane-system.json`
    - The binary and the config file are both called `plane-system`, which
      causes some issues if they are in the same directory and you don't
      explicitly specify the JSON file

## faq

>  Why don't I see any output?

You probably forgot to set the [`RUST_LOG`](https://docs.rs/env_logger/latest/env_logger/) environment variable.
