use anyhow::bail;

use serde::Deserialize;

pub mod custom;
pub mod preview;

/// Controls the plane system's interface with GStreamer, which can be used to
/// save video to files and livestream video to the ground.
#[derive(Clone, Debug, Deserialize)]
pub struct LivestreamConfig {
    pub custom: Option<custom::CustomConfig>,
    pub preview: Option<preview::PreviewConfig>,
}

pub fn create_tasks(
    config: LivestreamConfig,
    frame_rx: Option<flume::Receiver<ps_main_camera::LiveFrame>>,
) -> anyhow::Result<(Option<custom::CustomTask>, Option<preview::PreviewTask>)> {
    if let LivestreamConfig {
        custom: None,
        preview: None,
    } = config
    {
        bail!("cannot configure streaming without any endpoints");
    }

    let custom = config.custom.map(custom::create_task).transpose()?;
    let preview = if let Some(config) = config.preview {
        if let Some(frame_rx) = frame_rx {
            Some(preview::create_task(config, frame_rx)?)
        } else {
            bail!("preview endpoint is enabled but there is no source of frames (is main camera enabled?)")
        }
    } else {
        None
    };

    Ok((custom, preview))
}
