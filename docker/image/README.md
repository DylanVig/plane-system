# How to create plane system compiler images

## Requirements

- Install `podman`

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
