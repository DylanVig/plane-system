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
`plane-system` requires a configuration file. 
Use a `.json` file in the `config` folder or
  create your own `.json` file with the proper rules outlined.

- When testing with the camera:
  - Ensure that the camera is plugged in and the current user has permissions to
    control the camera. You can either run as root (not ideal) or create a
    `udev` rule to give your user access to the camera.
    - To create a udev rule to give proper permissions:
      -First in WSL, cd to /etc/udev/rules.d

      -Then, create a new udev rule file named 90-plane-system.rules and put in it:

      ```
      SUBSYSTEMS=="usb", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="0a79", MODE="0666", GROUP="plugdev"
      SUBSYSTEMS=="usb_device", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="0a79", MODE="0666", GROUP="plugdev"
      ```
  - Make sure you have `"main_camera": { "kind": "R10C" }` in your configuration file
- If you want to test with the gimbal:
  - Ensure that the gimbal is plugged in.
  - Make sure you have `"gimbal": true` in your configuration file
- Start the plane server:
  - In development mode, w/ source code available: `RUST_LOG=plane_system=debug cargo run`
  - In production, w/ just the binary: `RUST_LOG=plane_system=info ./plane-system --config=config/<config>.json`
    - The binary and the config file are both called `plane-system`, which
      causes some issues if they are in the same directory and you don't
      explicitly specify the JSON file

## faq

>  Why don't I see any output?

You probably forgot to set the [`RUST_LOG`](https://docs.rs/env_logger/latest/env_logger/) environment variable.

## troubleshooting for linux / wsl

> I got the error "No package 'glib-2.0' found "

You are missing packages that gstreamer needs: 

sudo apt install libglib2.0-dev libgstreamer1.0-dev   

> I got the error "thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: "Could not run `\"pkg-config\" \"--libs\" \"--cflags\" \"libudev\..."

Full Error:
"thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: "Could not run `\"pkg-config\" \"--libs\" \"--cflags\" \"libudev\"`\nThe pkg-config command could not be found.\n\nMost likely, you need to install a pkg-config package for your OS.\nTry `apt install pkg-config`, or `yum install pkg-config`,\nor `pkg install pkg-config` depending on your distribution.\n\nIf you've already installed it, ensure the pkg-config command is one of the\ndirectories in the PATH environment variable.\n\nIf you did not expect this build to link to a pre-installed system library,\nthen check documentation of the libudev-sys crate for an option to\nbuild the library from source, or disable features or dependencies\nthat require pkg-config."

Install the `libudev` Package:

```
sudo apt install libudev-dev
```

> The plane system only runs when I use sudo (WSL)

WSL by default does not have access to usb ports in the computer so it cannot connect to the camera when running. To fix this, we use a package, usbipd, that allows us to represent usb ports on the computer as servers that wsl can then contact.

In a PowerShell terminal, download [`usbipd`](https://github.com/dorssel/usbipd-win):

```
winget install usbipd
```

Make sure the camera is connected by usb

Find the camera's specific VID and PID by running:

```
usbipd list
```

Create and connect the camera to the server:

```
usbipd wsl attach -i 054c:0a79
```
