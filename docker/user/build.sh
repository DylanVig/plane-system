#!/bin/bash

DOCKER="${DOCKER:-docker}"

# copied from https://stackoverflow.com/a/246128/3508956
SCRIPT_DIR="$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
ROOT_DIR=$( dirname `dirname ${SCRIPT_DIR}` )

case "$1" in
"rpi3-raspbian-v1" | "rpi4-ubuntu-v1")
  echo "building plane system for target $1" 
  $DOCKER run -it --rm -v ${ROOT_DIR}:/app -v ~/.cargo/registry:/home/ccuser/.cargo/registry:z -v ~/.cargo/git:/home/ccuser/.cargo/git:z dr.cuair.org/x-compiler:$1
  ;;
"")
  echo "usage: build.sh <target>"
  ;;
*)
  echo "target must be one of: rpi3-raspbian-v1, rpi4-ubuntu-v1\n"
  ;;
esac
