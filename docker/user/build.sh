#!/bin/bash

set -euxo pipefail

DOCKER="${DOCKER:-docker}"

# copied from https://stackoverflow.com/a/246128/3508956
SCRIPT_DIR="$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
ROOT_DIR=$( dirname `dirname ${SCRIPT_DIR}` )

if ! podman volume exists plane-system-cargo; then 
  $DOCKER volume create plane-system-cargo
fi
if ! podman volume exists plane-system-build; then 
  $DOCKER volume create plane-system-build
fi

case "$1" in
"rpi3-raspbian-v1" | "rpi4-ubuntu-v1")
  echo "building plane system for target $1" 
  $DOCKER run -it --rm -v ${ROOT_DIR}:/app -v plane-system-build:/app/target -v plane-system-cargo:/home/ccuser/.cargo/registry dr.cuair.org/x-compiler:$1
  ;;
"")
  echo "usage: build.sh <target>"
  ;;
*)
  echo "target must be one of: rpi3-raspbian-v1, rpi4-ubuntu-v1"
  ;;
esac
