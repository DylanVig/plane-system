use clap::{AppSettings, Subcommand};

#[derive(Subcommand, Debug, Clone)]
#[clap(setting(AppSettings::NoBinaryName))]
#[clap(rename_all = "kebab-case")]
pub enum LivestreamRequest {
    Start { name: String },
    Stop { name: String },
}

pub type LivestreamResponse = ();
