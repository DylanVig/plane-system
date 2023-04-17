#!/bin/bash

set -euxo pipefail

runtime="${RUNTIME:-podman}"

# we default PARALLELISM=2 b/c M1/M2 Macs tend to come with small amounts of memory
# and swap configured, so they will just kill our processes if they use too much
# memory.

parallelism="${PARALLELISM:-2}"

# This script does NOT build the plane system. Instead, it builds a Runtime image
# which can be used to build the plane system for a Raspberry Pi 4.



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
	--build-arg BASE_IMAGE="docker.io/arm64v8/debian:bullseye" \
	--build-arg RUST_IMAGE="docker.io/arm64v8/rust:1.68-slim-bullseye" \
	-t dr.cuair.org/x-compiler/rpi4-ubuntu-arm64:v1 .
