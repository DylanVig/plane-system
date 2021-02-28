#!/bin/bash

TARGET_PREFIX="aarch64-linux-gnu-"
TARGET_ARCH="aarch64-unknown-linux-gnu"
TARGET_IP="192.168.1.239"

while getopts "a:" arg; do
  case "${arg}" in
    a)
      TARGET_ARCH=$OPTARG
      ;;
    i)
      TARGET_IP=$OPTARG
      ;;
    p)
      TARGET_PREFIX=$OPTARG
      ;;
  esac
done

echo "building executable"
cargo build --target=${TARGET_ARCH} --release || exit
echo "reducing executable size"
${TARGET_PREFIX}strip ./target/${TARGET_ARCH}/release/plane-system || exit
echo "copying executable"
scp ./target/${TARGET_ARCH}/release/plane-system ubuntu@${TARGET_IP}:/home/ubuntu/plane-system || exit
echo "running executable"
ssh ubuntu@${TARGET_IP} "RUST_LOG=plane_system=debug RUST_LOG_STYLE=always /home/ubuntu/plane-system --config plane-system.json" 
