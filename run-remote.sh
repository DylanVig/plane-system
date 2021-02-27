#!/bin/bash

TARGET_ARCH="aarch64-unknown-linux-gnu"
TARGET_IP="192.168.1.239"

while getopts "h:s:" arg; do
  case "${arg}" in
    a)
      TARGET_ARCH=$OPTARG
      ;;
  esac
done

cargo build --target=${TARGET_ARCH} || exit
echo "copying executable"
scp ./target/${TARGET_ARCH}/debug/plane-system ubuntu@${TARGET_IP}:/home/ubuntu/plane-system
echo "running executable"
ssh ubuntu@${TARGET_IP} "plane-system --config plane-system.json" 
