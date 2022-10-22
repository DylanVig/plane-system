# `@cuair/plane-system`

## how to compile

The plane system is designed to run on Unix. On Windows, you will need to use WSL, so skip to the [WSL setup](#wsl-setup) section.

- Install [Rust 1.63.0 or higher](https://rustup.rs/).
- Install dependencies
  - Arch/Manjaro: `pacman -S libusb gstreamer`
  - Ubuntu/Debian: `apt-get install libusb-1.0-0-dev libudev-dev libglib2.0-dev libgstreamer1.0-dev`
  - macOS: `brew install libusb gstreamer`
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

### when debugging
```
RUST_LOG=plane_system=debug cargo run -- --config=config/<config>.json
```

`plane-system` requires a configuration file. Use a `.json` file in the `config`
folder or create your own `.json` file with the proper rules outlined.

### in production
```
RUST_LOG=plane_system=info plane-system --config=<path/to/config>.json
```
## wsl setup

1. Make sure all of the Ubuntu packages from the [how to compile](#how-to-compile) section are installed in WSL.
2. Install `usbipd` in Windows:
    ```cmd
    winget install usbipd
    ```
3. Restart your computer.
4. If McAfee or other antivirus/firewalls besides Windows Defender are installed
   on your computer, disable these firewalls.
5. In WSL, create a file `/etc/udev/rules.d/90-plane-system.rules`:
    ```
    SUBSYSTEMS=="usb", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="0a79", MODE="0666", GROUP="plugdev"
    SUBSYSTEMS=="usb_device", ATTRS{idVendor}=="054c", ATTRS{idProduct}=="0a79", MODE="0666", GROUP="plugdev"
    ```
6. Start `udev` in WSL. **You will need to run this command again the first time you open a WSL terminal after every time you boot your computer. `udev` must be running in WSL when you plug in the camera, or else the plane system will not work properly.**
   ```
   sudo service udev start
   ```
   It may display a warning and force you to wait 60 seconds. 
7. Plug the Sony R10C camera into your computer via USB.
8. In PowerShell/CMD, use `usbipd` to make the camera accessible to Linux. **You will need to run this command again every time you plug in the camera.**
   ```
   usbipd wsl attach -i 054c:0a79
   ```
   After you run this command, you can check to make sure that `usbipd` is functioning properly by running `lsusb` in WSL. If it is, it will display an entry like this:
   ```
   Bus 001 Device 004: ID 054c:0a79 Sony Corp. UMC-R10C
   ```
9. Run the plane system as described in the [how to run](#how-to-run) section.
    
## troubleshooting runtime issues

### the plane system shows no output on the console

You probably forgot to set the
[`RUST_LOG`](https://docs.rs/env_logger/latest/env_logger/) environment
variable. Set it to `RUST_LOG=plane_system=debug` for a sane default.

## troubleshooting build errors

### `cargo build` fails with something relating to `pkg-config`

This means that you have not installed all of the necessary non-Rust dependencies of this project. This project depends on `libusb`, `libudev`, and `gstreamer`. `gstreamer` depends on `glib-2.0`.
