use clap::AppSettings;
use serde::Serialize;
use structopt::StructOpt;

use crate::Command;

pub type StreamCommand = Command<StreamRequest, StreamResponse>;

#[derive(StructOpt, Debug, Clone)]
#[structopt(setting(AppSettings::NoBinaryName))]
#[structopt(rename_all = "kebab-case")]
pub enum StreamRequest {
    Start {},
    End {},
}

#[derive(Debug, Clone, Serialize)]
pub enum StreamResponse {
    Unit,
}
