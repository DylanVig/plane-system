use clap::Subcommand;
use serde::Serialize;

use crate::Command;

pub type GimbalCommand = Command<GimbalRequest, GimbalResponse>;

#[derive(Subcommand, Debug, Clone)]
#[clap(rename_all = "kebab-case")]
pub enum GimbalRequest {
    Control { roll: f64, pitch: f64 },
}

#[derive(Debug, Clone, Serialize)]
pub enum GimbalResponse {
    Unit,
}