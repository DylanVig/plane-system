use serde::Deserialize;

use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize)]
pub struct DownloadConfig {
    /// The folder in which to save downloaded images
    pub save_path: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LiveConfig {
    /// The framerate at which the camera's live preview should be requested.
    /// Must be greater than zero and less than or equal to 30.
    pub framerate: f32,
}

#[derive(Debug, Deserialize)]
pub struct MainCameraConfig {
    #[cfg(feature = "csb")]
    pub current_sensing: Option<ps_main_camera_csb::CurrentSensingConfig>,

    pub download: DownloadConfig,

    pub live: Option<LiveConfig>,

    /// Minimium focal length of power zoom
    pub min_focal_length: Option<f32>,

    /// Maximium focal length of power zoom
    pub max_focal_length: Option<f32>,
}

pub struct PowerZoomConfig {}
