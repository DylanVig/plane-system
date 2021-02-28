#!/bin/bash

TARGET_ARCH="aarch64-unknown-linux-gnu"
TARGET_IP="192.168.1.239"

while getopts "a:" arg; do
  case "${arg}" in
    a)
      TARGET_ARCH=$OPTARG
      ;;
    i)
      TARGET_ARCH=$OPTARG
      ;;
  esac
done

echo "building executable"
cargo build --target=${TARGET_ARCH} --release|| exit
echo "reducing executable size"
strip ./target/${TARGET_ARCH}/debug/plane-system
echo "copying executable"
scp ./target/${TARGET_ARCH}/debug/plane-system ubuntu@${TARGET_IP}:/home/ubuntu/plane-system
echo "running executable"
ssh ubuntu@${TARGET_IP} "RUST_LOG=plane_system=debug /home/ubuntu/plane-system --config plane-system.json" 
