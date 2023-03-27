use serde::Deserialize;

use std::path::PathBuf;

use crate::GimbalKind;

#[derive(Debug, Deserialize)]
pub struct GimbalConfig {
    pub kind: GimbalKind,
    pub path: Option<String>,
}
