use anyhow::Context;
use async_trait::async_trait;
use log::*;
use ps_client::{ChannelCommandSink, ChannelCommandSource, Task};
use tokio::select;
use tokio_util::sync::CancellationToken;

use super::interface::*;
use super::*;

pub struct SaveTask {
    cmd_tx: ChannelCommandSink<SaveRequest, SaveResponse>,
    cmd_rx: ChannelCommandSource<SaveRequest, SaveResponse>,
    interface: SaveInterface,
}

pub fn create_task(config: SaveConfig) -> anyhow::Result<SaveTask> {
    let (cmd_tx, cmd_rx) = flume::bounded(256);

    if !config.path.exists() {
        std::fs::create_dir(&config.path).context("failed to create save directory")?;
    }

    let interface = SaveInterface::new(config.path, config.cameras)
        .context("failed to create save interface")?;

    Ok(SaveTask {
        interface,
        cmd_rx,
        cmd_tx,
    })
}

impl SaveTask {
    pub fn cmd(&self) -> ChannelCommandSink<SaveRequest, SaveResponse> {
        self.cmd_tx.clone()
    }
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
            ..
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
