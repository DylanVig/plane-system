#!/bin/bash

# This script does NOT build the plane system. Instead, it builds a Docker image
# which can be used to build the plane system for a Raspberry Pi 4.

docker build \
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
	-t dr.cuair.org/x-compiler:rpi4-ubuntu-v1 .
