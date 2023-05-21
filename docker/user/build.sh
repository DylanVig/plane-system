#!/bin/bash

# This script is used to compile the plane system using a cross-compiler image.

set -euxo pipefail

RUNTIME="${RUNTIME:-podman}"
CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"

# copied from https://stackoverflow.com/a/246128/3508956
SCRIPT_DIR="$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
ROOT_DIR=$( dirname `dirname ${SCRIPT_DIR}` )

case "$1" in
"rpi3-raspbian" | "rpi4-ubuntu")
  echo "building plane system for target $1" 
  $RUNTIME run -it --rm -v ${ROOT_DIR}:/app -v ${CARGO_HOME}/registry:/usr/local/cargo/registry -v ${CARGO_HOME}/git:/usr/local/cargo/git dr.cuair.org/x-compiler/$1:v1
  ;;
"")
  echo "usage: build.sh <target>"
  ;;
*)
  echo "target must be one of: rpi3-raspbian, rpi4-ubuntu"
  ;;
esac
