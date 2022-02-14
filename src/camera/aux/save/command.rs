use clap::StructOpt;
use clap::{AppSettings, Subcommand};
use serde::Serialize;

use crate::Command;

pub type SaveCommand = Command<SaveRequest, SaveResponse>;

#[derive(Subcommand, Debug, Clone)]
#[clap(setting(AppSettings::NoBinaryName))]
#[clap(rename_all = "kebab-case")]
pub enum SaveRequest {
    Start {},
    End {},
}

#[derive(Debug, Clone, Serialize)]
pub enum SaveResponse {
    Unit,
}
