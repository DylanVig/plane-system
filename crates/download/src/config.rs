use std::path::PathBuf;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct DownloadConfig {
    /// The folder in which to save downloaded images
    pub save_path: PathBuf,
}
