use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::bail;
use clap::Subcommand;
use serde::Serialize;

use super::{interface::OperatingMode, state::*};

#[derive(Subcommand, Debug, Clone)]
pub enum CameraRequest {
    /// view information about the storage media inside of the camera
    #[clap(subcommand)]
    Storage(CameraStorageRequest),

    /// view information about the files stored on the camera; download files
    #[clap(subcommand)]
    File(CameraFileRequest),

    /// capture an image
    Capture {
        /// if specified, will capture a burst of images over the given number
        /// of seconds
        #[clap(visible_alias = "burst", short, long)]
        burst_duration: Option<u8>,

        #[clap(visible_alias = "fast", short = 'f', long)]
        burst_high_speed: bool,
    },

    CCHack {
        #[clap(short, long)]
        interval: u8,

        #[clap(short, long)]
        count: Option<u32>,
    },

    Reset,

    Initialize,

    Status,
    ///initialize zoom to be able to set it using fov
    #[clap(subcommand)]
    ZoomInitialize(CameraZoomInitializeRequest),
    /// get a property of the camera's state
    #[clap(subcommand)]
    Get(CameraGetRequest),

    /// set a property of the camera's state
    #[clap(subcommand)]
    Set(CameraSetRequest),

    /// control continuous capture
    #[clap(name = "cc")]
    #[clap(subcommand)]
    ContinuousCapture(CameraContinuousCaptureRequest),

    /// record videos
    #[clap(subcommand)]
    Record(CameraRecordRequest),

    /// zoom in and out
    #[clap(subcommand)]
    Zoom(CameraZoomRequest),
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraZoomRequest {
    ///Zooms out for given milliseconds
    Wide { duration: u64 },
    ///Zooms in for given milliseconds
    Tele { duration: u64 },
    ///Zooms to level between [0,30] with 0 being fully zoomed out and 30 fully zoomed in.
    ///Warning: Zoom levels [30,60] do not physically zoom the camera but rather crop the image
    Level { level: u8 },
    ///Zooms to the closest focal length avaliable by the camera
    FocalLength { focal_length: f32 },
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraGetRequest {
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
pub enum CameraSetRequest {
    ExposureMode { mode: ExposureMode },
    OperatingMode { mode: OperatingMode },
    SaveMode { mode: SaveMedia },
    FocusMode { mode: FocusMode },
    CcInterval { interval: f32 },
    ShutterSpeed { speed: ShutterSpeed },
    Aperture { aperture: Aperture },
    // #[clap(external_subcommand)]
    // Other(Vec<String>),
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraZoomInitializeRequest {
    ///JSON to load magnification information from
    Load { path: PathBuf },
    ///Initialize magnification information (pairing zoom levels to fov) and save to file
    Initialize { path: PathBuf },
}
#[derive(Subcommand, Debug, Clone)]
pub enum CameraStorageRequest {
    /// list the storage volumes available on the camera
    List,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraFileRequest {
    /// list the files available on the camera
    List {
        /// the hexadecimal file handle of a folder; if provided, the contents
        /// of the folder will be listed
        #[clap(parse(try_from_str = ps_serde_util::parse_hex_u32))]
        parent: Option<u32>,
    },

    /// download a file from the camera
    Get {
        /// the hexadecimal file handle of a file
        #[clap(parse(try_from_str = ps_serde_util::parse_hex_u32))]
        handle: u32,
    },
}

impl FromStr for ExposureMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "m" | "manual" | "manual-exposure" => Ok(ExposureMode::ManualExposure),
            "p" | "program-auto" => Ok(ExposureMode::ProgramAuto),
            "a" | "aperture" | "aperture-priority" => Ok(ExposureMode::AperturePriority),
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
pub enum CameraContinuousCaptureRequest {
    Start,
    Stop,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraRecordRequest {
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
pub enum CameraResponse {
    Unit,
    Data {
        data: Vec<u8>,
    },
    Download {
        name: String,
    },
    StorageInfo {
        storages: HashMap<ptp::StorageId, ptp::StorageInfo>,
    },
    ObjectInfo {
        objects: HashMap<ptp::ObjectHandle, ptp::ObjectInfo>,
    },
    ZoomLevel(u8),
    CcInterval(f32),
    SaveMode(SaveMedia),
    OperatingMode(OperatingMode),
    ExposureMode(ExposureMode),
    FocusMode(FocusMode),
}
