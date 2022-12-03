pub mod command;
pub mod task;

use std::{collections::HashMap, path::PathBuf};

pub use command::*;
use serde::Deserialize;
pub use task::*;

#[derive(Clone, Debug, Deserialize)]
pub struct CustomConfig {
    /// Path where videos from the custom pipelines should be saved. The plane
    /// system will automatically create a folder named after the current time
    /// inside of this path and save videos here.
    pub save_path: PathBuf,

    /// Each key is the name of a pipeline that can be started at runtime, and
    /// each value is a GStreamer [pipeline
    /// description](https://gstreamer.freedesktop.org/documentation/tools/gst-launch.html?gi-language=c#pipeline-description).
    /// 
    /// Example:
    /// ```json
    /// "livestream": {
    ///   "custom": {
    ///     "save_path": "./videos/",
    ///     "pipelines": {
    ///       "keem": [
    ///         "v4l2src device=\"/dev/video0\" ! videoconvert ! x264enc ! mp4mux ! filesink={save_path}/out.mp4"
    ///       ]
    ///     }
    ///   }
    /// }
    /// ```
    /// 
    /// At runtime, you can enter into the plane system:
    /// ```text
    /// ps> livestream start keem
    /// ps> livestream stop keem
    /// ```
    pub pipelines: HashMap<String, Vec<String>>,
}
