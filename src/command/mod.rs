use std::collections::HashMap;

use clap::AppSettings;
use serde::Serialize;
use structopt::StructOpt;

#[derive(StructOpt, Debug, Clone)]
#[structopt(setting(AppSettings::NoBinaryName))]
#[structopt(rename_all = "kebab-case")]
pub enum CommandData {
    Camera(CameraCommand),
    Exit,
}

#[derive(Debug, Clone, Serialize)]
pub enum ResponseData {
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

#[derive(StructOpt, Debug, Clone)]
pub enum CameraCommand {
    #[structopt(name = "cd")]
    ChangeDirectory {
        directory: String,
    },

    Storage(CameraStorageCommand),

    File(CameraFileCommand),

    Capture,

    Power(CameraPowerCommand),

    Reconnect,

    Zoom {
        level: u8,
    },

    Download {
        file: Option<String>,
    },
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraStorageCommand {
    List,
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraFileCommand {
    List,
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraPowerCommand {
    Up,
    Down,
}
