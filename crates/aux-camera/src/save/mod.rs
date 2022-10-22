pub mod task;
pub mod command;
mod interface;

use std::path::PathBuf;

pub use task::*;
pub use command::*;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct SaveConfig {
    /// Path where videos from auxiliary cameras should be saved.
    pub path: PathBuf,

    /// A list of gstreamer camera specifications
    pub cameras: Vec<String>,
}
