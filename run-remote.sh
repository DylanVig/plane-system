#!/bin/bash

TARGET_PREFIX="aarch64-linux-gnu-"
TARGET_ARCH="aarch64-unknown-linux-gnu"
TARGET_IP="192.168.7.188"
TARGET_USER="ubuntu"

while getopts "a:i:p:" arg; do
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
scp ./target/${TARGET_ARCH}/release/plane-system ${TARGET_USER}@${TARGET_IP}:/home/${TARGET_USER}/plane-system || exit
echo "running executable"
ssh ${TARGET_USER}@${TARGET_IP} "RUST_LOG=plane_system=debug RUST_LOG_STYLE=always /home/${TARGET_USER}/plane-system --config plane-system.json" 
