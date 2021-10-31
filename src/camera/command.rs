use std::{collections::HashMap, str::FromStr};

use serde::Serialize;
use structopt::StructOpt;

use crate::Command;

use super::{interface::CameraOperatingMode, state::*};

pub type CameraCommand = Command<CameraCommandRequest, CameraCommandResponse>;

#[derive(StructOpt, Debug, Clone)]
pub enum CameraCommandRequest {
    /// view information about the storage media inside of the camera
    Storage(CameraCommandStorageRequest),

    /// view information about the files stored on the camera; download files
    File(CameraCommandFileRequest),

    /// capture an image
    Capture,

    /// disconnect and reconnect to the camera
    Reconnect,

    /// get a property of the camera's state
    Get(CameraCommandGetRequest),

    /// set a property of the camera's state
    Set(CameraCommandSetRequest),

    /// control continuous capture
    #[structopt(name = "cc")]
    ContinuousCapture(CameraCommandContinuousCaptureRequest),

    /// record videos
    Record(CameraCommandRecordRequest),
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraCommandGetRequest {
    ExposureMode,
    OperatingMode,
    SaveMode,
    FocusMode,
    ZoomLevel,
    CcInterval,

    #[structopt(external_subcommand)]
    Other(Vec<String>),
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraCommandSetRequest {
    ExposureMode {
        mode: CameraExposureMode,
    },
    OperatingMode {
        mode: CameraOperatingMode,
    },
    SaveMode {
        mode: CameraSaveMode,
    },
    FocusMode {
        mode: CameraFocusMode,
    },
    ZoomLevel {
        level: u16,
    },
    CcInterval {
        interval: f32,
    },

    #[structopt(external_subcommand)]
    Other(Vec<String>),
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraCommandStorageRequest {
    /// list the storage volumes available on the camera
    List,
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraCommandFileRequest {
    /// list the files available on the camera
    List {
        /// the hexadecimal file handle of a folder; if provided, the contents
        /// of the folder will be listed
        #[structopt(parse(try_from_str = crate::util::parse_hex_u32))]
        parent: Option<u32>,
    },

    /// download a file from the camera
    Get {
        /// the hexadecimal file handle of a file
        #[structopt(parse(try_from_str = crate::util::parse_hex_u32))]
        handle: u32,
    },
}

impl FromStr for CameraExposureMode {
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

impl FromStr for CameraSaveMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "host" | "host-device" => Ok(CameraSaveMode::HostDevice),
            "cam" | "camera" => Ok(CameraSaveMode::MemoryCard1),
            _ => bail!("invalid camera save mode"),
        }
    }
}

impl FromStr for CameraZoomMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "o" | "optical" => Ok(CameraZoomMode::Optical),
            "od" | "optical-digital" => Ok(CameraZoomMode::OpticalDigital),
            _ => bail!("invalid camera zoom mode"),
        }
    }
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraCommandContinuousCaptureRequest {
    Start,
    Stop,
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraCommandRecordRequest {
    Start,
    Stop,
}

impl FromStr for CameraOperatingMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "standby" => Self::Standby,
            "still" | "image" => Self::StillRec,
            "movie" | "video" => Self::MovieRec,
            "transfer" => Self::ContentsTransfer,
            _ => bail!("invalid operating mode"),
        })
    }
}

impl FromStr for CameraFocusMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "manual" | "m" => Self::Manual,
            "afc" => Self::AutoFocusContinuous,
            "afs" => Self::AutoFocusStill,
            _ => bail!("invalid focus mode"),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum CameraCommandResponse {
    Unit,
    Data {
        data: Vec<u8>,
    },
    Download {
        name: String,
    },
    StorageInfo {
        storages: HashMap<ptp::StorageId, ptp::PtpStorageInfo>,
    },
    ObjectInfo {
        objects: HashMap<ptp::ObjectHandle, ptp::PtpObjectInfo>,
    },
    ZoomLevel(u8),
    CcInterval(f32),
    SaveMode(CameraSaveMode),
    OperatingMode(CameraOperatingMode),
    ExposureMode(CameraExposureMode),
    FocusMode(CameraFocusMode),
}
