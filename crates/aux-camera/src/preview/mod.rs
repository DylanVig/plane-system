pub mod task;

use std::path::PathBuf;

use serde::Deserialize;
pub use task::*;

#[derive(Clone, Debug, Deserialize)]
pub struct PreviewConfig {
    /// A list of gstreamer pipeline specifications.
    pub pipelines: Vec<String>,
}
