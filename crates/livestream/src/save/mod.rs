pub mod command;
mod interface;
pub mod task;

use std::path::PathBuf;

pub use command::*;
use serde::Deserialize;
pub use task::*;

#[derive(Clone, Debug, Deserialize)]
pub struct SaveConfig {
    /// Path where videos from auxiliary cameras should be saved
    pub save_path: PathBuf,

    /// File extension of output videos.
    pub save_ext: String,

    /// A list of gstreamer pipeline specifications. A filesink will be appended
    /// to the end of each one.
    pub pipelines: Vec<String>,
}
