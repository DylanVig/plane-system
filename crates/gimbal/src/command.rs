use clap::{AppSettings, Subcommand};

#[derive(Subcommand, Debug, Clone)]
#[clap(setting(AppSettings::NoBinaryName))]
#[clap(rename_all = "kebab-case")]
pub enum GimbalRequest {
    Debug { roll: f32, pitch: f32 },
}

pub type GimbalResponse = ();
