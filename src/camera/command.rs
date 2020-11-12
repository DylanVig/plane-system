use std::collections::HashMap;

use serde::Serialize;
use structopt::StructOpt;

use crate::Command;

use super::state::*;

pub type CameraCommand = Command<CameraRequest, CameraResponse>;

#[derive(StructOpt, Debug, Clone)]
pub enum CameraRequest {
    Storage(CameraStorageRequest),

    File(CameraFileRequest),

    Capture,

    Power(CameraPowerRequest),

    Reconnect,

    Zoom(CameraZoomRequest),

    Exposure(CameraExposureRequest),

    SaveMode(CameraSaveModeRequest),

    Reset,
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraStorageRequest {
    List,
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraFileRequest {
    List {
        #[structopt(parse(try_from_str = crate::util::parse_hex_u32))]
        parent: Option<u32>,
    },
    Get {
        #[structopt(parse(try_from_str = crate::util::parse_hex_u32))]
        handle: u32,
    },
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraExposureRequest {
    Mode(CameraExposureModeRequest),
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraExposureModeRequest {
    Get,
    Set { mode: CameraExposureMode },
}

impl std::str::FromStr for CameraExposureMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "m" | "manual" | "manual-exposure" => Ok(CameraExposureMode::ManualExposure),
            "p" | "program-auto" => Ok(CameraExposureMode::ProgramAuto),
            "a" | "aperture" | "apeture-priority" => Ok(CameraExposureMode::AperturePriority),
            "s" | "shutter" | "shutter-priority" => Ok(CameraExposureMode::ShutterPriority),
            "i" | "intelligent-auto" => Ok(CameraExposureMode::IntelligentAuto),
            "superior-auto" => Ok(CameraExposureMode::SuperiorAuto),
            "movie-program-auto" => Ok(CameraExposureMode::MovieProgramAuto),
            "movie-aperture-priority" => Ok(CameraExposureMode::MovieAperturePriority),
            "movie-shutter-priority" => Ok(CameraExposureMode::MovieShutterPriority),
            "movie-manual-exposure" => Ok(CameraExposureMode::MovieManualExposure),
            "movie-intelligent-auto" => Ok(CameraExposureMode::MovieIntelligentAuto),
            _ => bail!("invalid camera exposure mode"),
        }
    }
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraSaveModeRequest {
    Get,
    Set { mode: CameraSaveMode },
}

impl std::str::FromStr for CameraSaveMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "host" | "host-device" => Ok(CameraSaveMode::HostDevice),
            "cam" | "camera" => Ok(CameraSaveMode::MemoryCard1),
            _ => bail!("invalid camera save mode"),
        }
    }
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraZoomRequest {
    Level(CameraZoomLevelRequest),
    Mode(CameraZoomModeRequest),
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraZoomLevelRequest {
    Get,
    Set { level: u8 },
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraZoomModeRequest {
    Optical,
    OpticalDigital,
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraPowerRequest {
    Up,
    Down,
}

#[derive(Debug, Clone, Serialize)]
pub enum CameraResponse {
    Unit,
    Data {
        data: Vec<u8>,
    },
    File {
        path: std::path::PathBuf,
    },
    StorageInfo {
        storages: HashMap<ptp::StorageId, ptp::PtpStorageInfo>,
    },
    ObjectInfo {
        objects: HashMap<ptp::ObjectHandle, ptp::PtpObjectInfo>,
    },
    ZoomLevel {
        zoom_level: u8,
    },
    SaveMode {
        save_mode: CameraSaveMode,
    },
    ExposureMode {
        exposure_mode: CameraExposureMode,
    },
}
