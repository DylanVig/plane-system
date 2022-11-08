use anyhow::bail;

use serde::Deserialize;

pub mod preview;
pub mod save;
pub mod stream;

#[derive(Clone, Debug, Deserialize)]
pub struct AuxCameraConfig {
    pub stream: Option<stream::StreamConfig>,
    pub save: Option<save::SaveConfig>,
    pub preview: Option<preview::PreviewConfig>,
}

pub fn create_tasks(
    config: AuxCameraConfig,
    frame_rx: Option<flume::Receiver<ps_main_camera::LiveFrame>>,
) -> anyhow::Result<(
    Option<stream::StreamTask>,
    Option<save::SaveTask>,
    Option<preview::PreviewTask>,
)> {
    if let AuxCameraConfig {
        stream: None,
        save: None,
        preview: None,
    } = config
    {
        bail!("cannot configure streaming without any endpoints");
    }

    let stream = config.stream.map(stream::create_task).transpose()?;
    let save = config.save.map(save::create_task).transpose()?;
    let preview = if let Some(config) = config.preview {
        if let Some(frame_rx) = frame_rx {
            Some(preview::create_task(config, frame_rx)?)
        } else {
            bail!("preview endpoint is enabled but there is no source of frames (is main camera enabled?)")
        }
    } else {
        None
    };

    Ok((stream, save, preview))
}
