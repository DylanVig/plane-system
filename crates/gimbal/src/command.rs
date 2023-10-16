use clap::Subcommand;
use serde::Serialize;
use thiserror::Error;

pub type GimbalCommand = ps_client::Command<GimbalRequest, GimbalResponse>;

#[derive(Subcommand, Debug, Clone)]
#[clap(rename_all = "kebab-case")]
pub enum GimbalRequest {
    Control {
        /// roll in degrees
        #[arg(allow_negative_numbers = true)]
        roll: f64,
        /// pitch in degrees
        #[arg(allow_negative_numbers = true)]
        pitch: f64,
    },
}

#[derive(Debug, Clone, Serialize)]
pub enum GimbalResponse {
    Unit,
}



#[derive(Error, Debug)]
pub enum GimbalError {
    #[error("could not send request to the gimbal")]
    GimbalRequestError,
}