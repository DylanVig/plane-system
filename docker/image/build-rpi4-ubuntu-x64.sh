#!/bin/bash

set -euxo pipefail

runtime="${RUNTIME:-podman}"
parallelism="${PARALLELISM:-8}"

# This script does NOT build the plane system. Instead, it builds a Runtime image
# which can be used to build the plane system for a Raspberry Pi 4.

# If you are unable to run this script, but no real error message is printed, it
# might be because you are running out of memory. The default parallelism is 8,
# meaning that the compiler runs 8 instances at a time, which could be causing
# your computer to run out of memory. Try running with PARALELLISM=2 and see if
# it works. The tradeoff is that it will take longer to create the image.

$runtime build \
	--build-arg PARALLELISM=$parallelism \
	--build-arg GLIBC_VERSION=2.32 \
	--build-arg BINUTILS_VERSION=2.37 \
	--build-arg LINUX_SERIES=5.x \
	--build-arg LINUX_VERSION=5.10.3 \
	--build-arg GCC_VERSION=8.5.0 \
	--build-arg TARGET_GCC=aarch64-linux-gnu \
	--build-arg TARGET_LINUX=arm64 \
	--build-arg TARGET_DEBIAN=arm64 \
	--build-arg TARGET_PKGCONFIG=aarch64-linux-gnu \
	--build-arg TARGET_RUST=aarch64-unknown-linux-gnu \
	--build-arg CPPFLAGS="" \
	-t dr.cuair.org/x-compiler/rpi4-ubuntu-amd64:v1 .
