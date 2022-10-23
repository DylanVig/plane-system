#!/bin/bash
RUST_LOG="plane_system=debug,ps_telemetry=debug,ps_aux_camera=debug,ps_main_camera=debug,ps_pixhawk=debug,ps_gs=debug,ps_download=debug" cargo run -- "$@"
