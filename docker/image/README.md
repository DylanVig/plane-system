# How to create plane system compiler images

The scripts in this folder create Podman/Docker images which can be used to
compile the plane system for a specific target. They contain the libraries that
the plane system needs to link against, as well as a cross-compiler that targets
the Raspberry Pi's CPU architecture and operating system.

## Requirements

- Install `podman`

## How to

Run one of the `build-rpi*-*-*.sh` scripts. 
- If you want to use Docker instead of Podman, you need to set `RUNTIME=docker` environment variable.
- These scripts compile GCC and Binutils from source. This takes a while. It can
  be made faster by increasing the parallelism of the build, but that will cause
  the build to require more memory. If you increase the parallelism too much,
  the memory requirements will be too high and the build will fail. This is
  especially a problem on macOS (see [macOS hurdles](#macos-hurdles))

## macOS hurdles

On MacOS, `podman` functions by creating a virtual Linux machine and then
running your containers inside of this Linux VM. The default RAM limit on this
VM is 2GB. However, to create our plane system compiler images, we need to
compile GCC and Binutils, and compiling them with just 2GB of RAM is impossible.
To increase the memory limit, run the following commands:

```bash
podman machine stop
podman machine rm
podman machine init --cpus 2 --memory 6144 # amount of memory in KBs, 6GB may be overkill
podman machine start
```

If the `podman` VM is already running, you'll need to restart it with `podman
machine stop` followed by `podman machine start`.
