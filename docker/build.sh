#!/bin/bash

# copied from https://stackoverflow.com/a/246128/3508956
SCRIPT_DIR="$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
ROOT_DIR=$( dirname ${SCRIPT_DIR} )

docker run -it -e CFLAGS="-mfpu=vfp3 -mfloat-abi=hard" -v ${ROOT_DIR}:/app -v ~/.cargo/registry:/home/ccuser/.cargo/registry -v ~/.cargo/git:/home/ccuser/.cargo/git dr.cuair.org/x-compiler:rpi3-raspbian-v1 bash
