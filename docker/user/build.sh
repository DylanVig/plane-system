#!/bin/bash

# This script is used to compile the plane system using a cross-compiler image.

set -euxo pipefail

RUNTIME="${RUNTIME:-podman}"

# copied from https://stackoverflow.com/a/246128/3508956
SCRIPT_DIR="$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
ROOT_DIR=$( dirname `dirname ${SCRIPT_DIR}` )

if ! podman volume exists plane-system-cargo; then 
  $RUNTIME volume create plane-system-cargo
fi
if ! podman volume exists plane-system-build; then 
  $RUNTIME volume create plane-system-build
fi

case "$1" in
"rpi3-raspbian" | "rpi4-ubuntu")
  echo "building plane system for target $1" 
  $RUNTIME run -it --rm  -v ${ROOT_DIR}:/app -v plane-system-build:/app/target -v plane-system-cargo:/home/ccuser/.cargo/registry dr.cuair.org/x-compiler/$1:v1
  ;;
"")
  echo "usage: build.sh <target>"
  ;;
*)
  echo "target must be one of: rpi3-raspbian, rpi4-ubuntu"
  ;;
esac
