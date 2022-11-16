pub mod command;
pub mod task;

use std::{collections::HashMap, path::PathBuf};

pub use command::*;
use serde::Deserialize;
pub use task::*;

#[derive(Clone, Debug, Deserialize)]
pub struct CustomConfig {
    /// Path where videos from auxiliary cameras should be saved
    pub save_path: PathBuf,

    /// A list of gstreamer pipeline specifications.
    pub pipelines: HashMap<String, Vec<String>>,
}
