use clap::AppSettings;
use serde::Serialize;
use structopt::StructOpt;

use crate::Command;

pub type GimbalCommand = Command<GimbalRequest, GimbalResponse>;

#[derive(StructOpt, Debug, Clone)]
#[structopt(setting(AppSettings::NoBinaryName))]
#[structopt(rename_all = "kebab-case")]
pub enum GimbalRequest {
    Control {
        roll: f64,
        pitch: f64,
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum GimbalResponse {
    Unit,
}
