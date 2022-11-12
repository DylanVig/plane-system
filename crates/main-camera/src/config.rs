use serde::Deserialize;

use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize)]
pub struct DownloadConfig {
    /// The folder in which to save downloaded images
    pub save_path: PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct MainCameraConfig {
    #[cfg(feature = "csb")]
    pub current_sensing: Option<ps_main_camera_csb::CurrentSensingConfig>,

    pub download: DownloadConfig,
}
