use anyhow::bail;
use ps_client::ChannelCommandSink;
use serde::Deserialize;

pub mod save;
pub mod stream;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub stream: Option<stream::StreamConfig>,
    pub save: Option<save::SaveConfig>,
}

pub fn create_tasks(
    config: Config,
) -> anyhow::Result<(
    Option<(
        stream::StreamTask,
        ChannelCommandSink<stream::StreamRequest, stream::StreamResponse>,
    )>,
    Option<(
        save::SaveTask,
        ChannelCommandSink<save::SaveRequest, save::SaveResponse>,
    )>,
)> {
    if config.stream.is_none() && config.save.is_none() {
        bail!("cannot configure auxiliary cameras without specifying stream settings and/or save settings");
    }

    let stream = config.stream.map(stream::create_task).transpose()?;
    let save = config.save.map(save::create_task).transpose()?;

    Ok((stream, save))
}
