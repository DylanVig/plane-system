use clap::StructOpt;
use clap::{AppSettings, Subcommand};
use serde::Serialize;

use crate::Command;

pub type StreamCommand = Command<StreamRequest, StreamResponse>;

#[derive(Subcommand, Debug, Clone)]
#[clap(setting(AppSettings::NoBinaryName))]
#[clap(rename_all = "kebab-case")]
pub enum StreamRequest {
    Start {},
    End {},
}

#[derive(Debug, Clone, Serialize)]
pub enum StreamResponse {
    Unit,
}
