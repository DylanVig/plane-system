pub mod task;

use std::path::PathBuf;

use serde::Deserialize;
pub use task::*;

#[derive(Clone, Debug, Deserialize)]
pub struct PreviewConfig {
    /// Path where videos from the camera preview should be saved. The plane
    /// system will automatically create a folder named after the current time
    /// inside of this path and save videos here.
    pub save_path: PathBuf,

    /// Describes a GStreamer
    /// [bin](https://gstreamer.freedesktop.org/documentation/application-development/basics/bins.html)
    /// which will received JPEG-encoded frames from the R10C via an `appsrc`.
    ///
    /// Strings are joined together with newlines. Can use `{save_path}` as a
    /// placeholder for the timestamped save path.
    pub bin: Vec<String>,
}
