use std::collections::HashMap;

use serde::Serialize;
use structopt::StructOpt;

use crate::Command;

pub type CameraCommand = Command<CameraRequest, CameraResponse>;

#[derive(StructOpt, Debug, Clone)]
pub enum CameraRequest {
    #[structopt(name = "cd")]
    ChangeDirectory {
        directory: String,
    },

    Storage(CameraStorageRequest),

    File(CameraFileRequest),

    Capture,

    Power(CameraPowerRequest),

    Reconnect,

    Zoom {
        level: u8,
    },

    Download {
        file: Option<String>,
    },
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraStorageRequest {
    List,
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraFileRequest {
    List {
        #[structopt(parse(try_from_str = crate::util::parse_hex_u32))]
        parent: Option<u32>
    },
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
    CameraStorageInfo {
        storages: HashMap<ptp::StorageId, ptp::PtpStorageInfo>,
    },
    CameraObjectInfo {
        objects: HashMap<ptp::ObjectHandle, ptp::PtpObjectInfo>,
    },
}
