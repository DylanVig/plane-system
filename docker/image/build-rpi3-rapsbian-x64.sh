#!/bin/bash

set -euxo pipefail

runtime="${RUNTIME:-podman}"
parallelism="${PARALLELISM:-8}"

# This script does NOT build the plane system. Instead, it builds a Runtime image
# which can be used to build the plane system for a Raspberry Pi 3. This
# cross-compiler uses glibc 2.28, so the binaries it generates will be able to
# run on Raspbian as well as Ubuntu.

# If you are unable to run this script, but no real error message is printed, it
# might be because you are running out of memory. The default parallelism is 8,
# meaning that the compiler runs 8 instances at a time, which could be causing
# your computer to run out of memory. Try running with PARALELLISM=2 and see if
# it works. The tradeoff is that it will take longer to create the image.

$runtime build \
	--build-arg PARALLELISM=$parallelism \
	--build-arg GLIBC_VERSION=2.28 \
	--build-arg BINUTILS_VERSION=2.37 \
	--build-arg LINUX_SERIES=5.x \
	--build-arg LINUX_VERSION=5.10.3 \
	--build-arg GCC_VERSION=8.5.0 \
	--build-arg GCC_MULTILIBS=aprofile \
	--build-arg GCC_CONFIGURE_FLAGS="--with-float=hard" \
	--build-arg TARGET_GCC=armv7l-linux-gnueabihf \
	--build-arg TARGET_LINUX=arm \
	--build-arg TARGET_DEBIAN=armhf \
	--build-arg TARGET_PKGCONFIG=arm-linux-gnueabihf \
	--build-arg TARGET_RUST=armv7-unknown-linux-gnueabihf \
	--build-arg CPPFLAGS="-mfloat-abi=hard -mfpu=vfp3 -march=armv7-a" \
	-t dr.cuair.org/x-compiler/rpi3-raspbian-amd64:v1 .
