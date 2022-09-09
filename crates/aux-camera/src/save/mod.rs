pub mod task;
pub mod command;
mod interface;

use std::path::PathBuf;

pub use task::*;
pub use command::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SaveConfig {
    pub save_path: PathBuf,
}
