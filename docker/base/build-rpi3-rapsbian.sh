#!/bin/bash

# This script does NOT build the plane system. Instead, it builds a Docker image
# which can be used to build the plane system for a Raspberry Pi 3. This
# cross-compiler uses glibc 2.28, so the binaries it generates will be able to
# run on Raspbian as well as Ubuntu.

docker build \
	--build-arg GLIBC_VERSION=2.28 \
	--build-arg BINUTILS_VERSION=2.37 \
	--build-arg LINUX_SERIES=5.x \
	--build-arg LINUX_VERSION=5.10.3 \
	--build-arg GCC_VERSION=8.5.0 \
	--build-arg GCC_MULTILIBS=aprofile \
	--build-arg TARGET_GCC=armv7l-linux-gnueabihf \
	--build-arg TARGET_LINUX=arm \
	--build-arg TARGET_RUST=armv7-unknown-linux-gnueabihf \
	--build-arg CFLAGS="-mfpu=vfp3" \
	-t dr.cuair.org/x-compiler:rpi3-raspbian-v1 .
