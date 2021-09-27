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

    /// control the camera's zoom lens
    Zoom(CameraCommandZoomRequest),

    /// control the camera's exposure mode
    Exposure(CameraCommandExposureRequest),

    /// control whether the camera saves to its internal storage or to the host
    SaveMode(CameraCommandSaveModeRequest),

    /// control continuous capture
    #[structopt(name = "cc")]
    ContinuousCapture(CameraCommandContinuousCaptureRequest),

    /// control operating mode
    #[structopt(name = "mode")]
    OperationMode(CameraCommandOperationModeRequest),

    #[structopt(name = "focus")]
    FocusMode(CameraCommandFocusModeRequest),

    /// record videos
    Record(CameraCommandRecordRequest),

    /// dump the state of the camera to the console
    Debug(CameraCommandDebugRequest),
}

#[derive(StructOpt, Debug, Clone)]
pub struct CameraCommandDebugRequest {
    #[structopt(parse(try_from_str = crate::util::parse_hex_u32))]
    pub property: Option<u32>,

    pub value_num: Vec<isize>,
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

#[derive(StructOpt, Debug, Clone)]
pub enum CameraCommandExposureRequest {
    Mode(CameraExposureModeRequest),
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraExposureModeRequest {
    /// get the current exposure mode
    Get,

    /// set the current exposure mode
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
pub enum CameraCommandSaveModeRequest {
    /// get the current save mode
    Get,

    /// set the current save mode
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
pub enum CameraCommandZoomRequest {
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
pub enum CameraCommandPowerRequest {
    Up,
    Down,
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraCommandContinuousCaptureRequest {
    Start,
    Stop,
    Interval { interval: f32 },
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraCommandRecordRequest {
    Start,
    Stop,
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraCommandOperationModeRequest {
    /// get the current exposure mode
    Get,

    /// set the current exposure mode
    Set { mode: CameraOperatingMode },
}

impl FromStr for CameraOperatingMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "standby" => Self::Standby,
            "still" | "image" => Self::StillRec,
            "movie" | "video" => Self::MovieRec,
            "transfer" => Self::ContentsTransfer,
            _ => bail!("invalid operating mode")
        })
    }
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraCommandFocusModeRequest {
    /// get the current focus mode
    Get,

    /// set the current focus mode
    Set { mode: CameraFocusMode },
}

impl FromStr for CameraFocusMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "manual" | "m" => Self::Manual,
            "afc" => Self::AutoFocusContinuous,
            "afs"  => Self::AutoFocusStill,
            _ => bail!("invalid focus mode")
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
    ZoomLevel {
        zoom_level: u8,
    },
    SaveMode {
        save_mode: CameraSaveMode,
    },
    OperatingMode {
        operating_mode: CameraOperatingMode,
    },
    ExposureMode {
        exposure_mode: CameraExposureMode,
    },
    FocusMode {
        focus_mode: CameraFocusMode,
    },
}
