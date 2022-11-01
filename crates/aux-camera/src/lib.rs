use anyhow::bail;

use serde::Deserialize;

pub mod save;
pub mod stream;

#[derive(Clone, Debug, Deserialize)]
pub struct AuxCameraConfig {
    pub stream: Option<stream::StreamConfig>,
    pub save: Option<save::SaveConfig>,
}

pub fn create_tasks(
    config: AuxCameraConfig,
) -> anyhow::Result<(Option<stream::StreamTask>, Option<save::SaveTask>)> {
    if config.stream.is_none() && config.save.is_none() {
        bail!("cannot configure auxiliary cameras without specifying stream settings and/or save settings");
    }

    let stream = config.stream.map(stream::create_task).transpose()?;
    let save = config.save.map(save::create_task).transpose()?;

    Ok((stream, save))
}
