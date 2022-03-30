use std::{collections::HashMap, str::FromStr};

use clap::Subcommand;
use serde::Serialize;

use crate::Command;

use super::{interface::OperatingMode, state::*};

pub type CameraCommand = Command<CameraCommandRequest, CameraCommandResponse>;

#[derive(Subcommand, Debug, Clone)]
pub enum CameraCommandRequest {
    /// view information about the storage media inside of the camera
    #[clap(subcommand)]
    Storage(CameraCommandStorageRequest),

    /// view information about the files stored on the camera; download files
    #[clap(subcommand)]
    File(CameraCommandFileRequest),

    /// capture an image
    Capture,

    /// disconnect and reconnect to the camera
    Reconnect,

    Status,

    /// get a property of the camera's state
    #[clap(subcommand)]
    Get(CameraCommandGetRequest),

    /// set a property of the camera's state
    #[clap(subcommand)]
    Set(CameraCommandSetRequest),

    /// control continuous capture
    #[clap(name = "cc")]
    #[clap(subcommand)]
    ContinuousCapture(CameraCommandContinuousCaptureRequest),

    /// control continuous capture
    #[clap(name = "lv")]
    #[clap(subcommand)]
    LiveView(CameraCommandLiveViewRequest),

    /// record videos
    #[clap(subcommand)]
    Record(CameraCommandRecordRequest),
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraCommandGetRequest {
    ExposureMode,
    OperatingMode,
    SaveMode,
    FocusMode,
    ZoomLevel,
    CcInterval,

    #[clap(external_subcommand)]
    Other(Vec<String>),
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraCommandSetRequest {
    ExposureMode {
        mode: ExposureMode,
    },
    OperatingMode {
        mode: OperatingMode,
    },
    SaveMode {
        mode: SaveMedia,
    },
    FocusMode {
        mode: FocusMode,
    },
    ZoomLevel {
        level: u16,
    },
    CcInterval {
        interval: f32,
    },
    ShutterSpeed {
        speed: ShutterSpeed,
    },
    Aperture {
        aperture: Aperture,
    },

    // #[clap(external_subcommand)]
    // Other(Vec<String>),
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraCommandStorageRequest {
    /// list the storage volumes available on the camera
    List,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraCommandFileRequest {
    /// list the files available on the camera
    List {
        /// the hexadecimal file handle of a folder; if provided, the contents
        /// of the folder will be listed
        #[clap(parse(try_from_str = crate::util::parse_hex_u32))]
        parent: Option<u32>,
    },

    /// download a file from the camera
    Get {
        /// the hexadecimal file handle of a file
        #[clap(parse(try_from_str = crate::util::parse_hex_u32))]
        handle: u32,
    },
}

impl FromStr for ExposureMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "m" | "manual" | "manual-exposure" => Ok(ExposureMode::ManualExposure),
            "p" | "program-auto" => Ok(ExposureMode::ProgramAuto),
            "a" | "aperture" | "apeture-priority" => Ok(ExposureMode::AperturePriority),
            "s" | "shutter" | "shutter-priority" => Ok(ExposureMode::ShutterPriority),
            "i" | "intelligent-auto" => Ok(ExposureMode::IntelligentAuto),
            "superior-auto" => Ok(ExposureMode::SuperiorAuto),
            "movie-program-auto" => Ok(ExposureMode::MovieProgramAuto),
            "movie-aperture-priority" => Ok(ExposureMode::MovieAperturePriority),
            "movie-shutter-priority" => Ok(ExposureMode::MovieShutterPriority),
            "movie-manual-exposure" => Ok(ExposureMode::MovieManualExposure),
            "movie-intelligent-auto" => Ok(ExposureMode::MovieIntelligentAuto),
            _ => bail!("invalid camera exposure mode"),
        }
    }
}

impl FromStr for SaveMedia {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "host" | "host-device" => Ok(SaveMedia::HostDevice),
            "cam" | "camera" => Ok(SaveMedia::MemoryCard1),
            _ => bail!("invalid camera save mode"),
        }
    }
}

impl FromStr for ZoomMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "o" | "optical" => Ok(ZoomMode::Optical),
            "od" | "optical-digital" => Ok(ZoomMode::OpticalDigital),
            _ => bail!("invalid camera zoom mode"),
        }
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraCommandContinuousCaptureRequest {
    Start,
    Stop,
}


#[derive(Subcommand, Debug, Clone)]
pub enum CameraCommandLiveViewRequest {
    Start,
    Stop,
}


#[derive(Subcommand, Debug, Clone)]
pub enum CameraCommandRecordRequest {
    Start,
    Stop,
}

impl FromStr for OperatingMode {
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

impl FromStr for FocusMode {
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
    SaveMode(SaveMedia),
    OperatingMode(OperatingMode),
    ExposureMode(ExposureMode),
    FocusMode(FocusMode),
}
