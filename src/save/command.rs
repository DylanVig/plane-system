use clap::AppSettings;
use serde::Serialize;
use structopt::StructOpt;

use crate::Command;

pub type SaveCommand = Command<SaveRequest, SaveResponse>;

#[derive(StructOpt, Debug, Clone)]
#[structopt(setting(AppSettings::NoBinaryName))]
#[structopt(rename_all = "kebab-case")]
pub enum SaveRequest {
  Start {},
  End {},
}

#[derive(Debug, Clone, Serialize)]
pub enum SaveResponse {
  Unit,
}
