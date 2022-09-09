use serde::Deserialize;

pub mod save;
pub mod stream;


#[derive(Debug, Deserialize)]
pub struct Config {
    pub stream: Option<save::SaveConfig>,
    pub save: Option<stream::StreamConfig>,

    // a list of gstreamer camera specifications
    pub cameras: Vec<String>,
}
