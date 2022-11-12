pub mod task;

use std::path::PathBuf;

use serde::Deserialize;
pub use task::*;

#[derive(Clone, Debug, Deserialize)]
pub struct PreviewConfig {
    save_path: PathBuf,

    /// A GStreamer bin specification. Will take an image/jpeg stream as input.
    /// Strings are joined with newlines in between to create the spec.
    pub bin_spec: Vec<String>,
}
