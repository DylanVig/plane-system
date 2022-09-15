use crate::Config;
use anyhow::Context;
use async_trait::async_trait;
use log::*;
use ps_client::{ChannelCommandSink, ChannelCommandSource, Task};
use tokio::select;
use tokio_util::sync::CancellationToken;

use super::interface::*;
use super::*;

pub struct SaveTask {
    cmd_rx: ChannelCommandSource<SaveRequest, SaveResponse>,
    interface: SaveInterface,
}

pub fn create_task(
    general_config: Config,
    save_config: SaveConfig,
) -> anyhow::Result<(SaveTask, ChannelCommandSink<SaveRequest, SaveResponse>)> {
    let (cmd_tx, cmd_rx) = flume::bounded(256);

    if !save_config.save_path.exists() {
        std::fs::create_dir(&save_config.save_path).context("failed to create save directory")?;
    }

    let interface = SaveInterface::new(save_config.save_path, general_config.cameras)
        .context("failed to create save interface")?;

    Ok((SaveTask { interface, cmd_rx }, cmd_tx))
}

#[async_trait]
impl Task for SaveTask {
    fn name(&self) -> &'static str {
        "aux-camera/save"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            mut interface,
            cmd_rx,
        } = *self;

        let cmd_loop = async {
            trace!("initializing saver");

            while let Ok((cmd, ret_tx)) = cmd_rx.recv_async().await {
                let result = tokio::task::block_in_place(|| match cmd {
                    SaveRequest::Start {} => interface.start_save(),
                    SaveRequest::End {} => interface.end_save(),
                });

                let _ = ret_tx.send(result);
            }

            Ok::<_, anyhow::Error>(())
        };

        select! {
            _ = cancel.cancelled() => {}
            res = cmd_loop => { res? }
        };

        Ok(())
    }
}
