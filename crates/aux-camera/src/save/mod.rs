pub mod command;
mod interface;
pub mod task;

use std::path::PathBuf;

pub use command::*;
use serde::Deserialize;
pub use task::*;

#[derive(Clone, Debug, Deserialize)]
pub struct SaveConfig {
    /// Path where videos from auxiliary cameras should be saved.
    pub path: PathBuf,

    /// A list of gstreamer camera specifications
    pub cameras: Vec<String>,
}
