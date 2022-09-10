use crate::Config;
use anyhow::Context;
use async_trait::async_trait;
use log::*;
use ps_client::{ChannelCommandSource, ChannelCommandSink, Task};
use tokio::select;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::interface::*;
use super::*;

pub struct SaveTask {
    general_config: Config,
    save_config: SaveConfig,
    cmd_rx: ChannelCommandSource<SaveRequest, SaveResponse>,
}

impl SaveTask {
    pub fn create(
        general_config: Config,
        save_config: SaveConfig,
    ) -> (Self, ChannelCommandSink<SaveRequest, SaveResponse>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(256);

        (
            Self {
                general_config,
                save_config,
                cmd_rx,
            },
            cmd_tx,
        )
    }
}

#[async_trait]
impl Task for SaveTask {
    async fn run(self, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            general_config,
            save_config,
            mut cmd_rx,
        } = self;

        let cmd_loop = async {
            let mut iface = SaveInterface::new(&save_config.save_path, general_config.cameras)
                .context("failed to create save interface")?;

            if !save_config.save_path.exists() {
                tokio::fs::create_dir(save_config.save_path)
                    .await
                    .context("failed to create save directory")?;
            }

            trace!("initializing saver");

            while let Some((cmd, ret_tx)) = cmd_rx.recv().await {
                let result = tokio::task::block_in_place(|| match cmd {
                    SaveRequest::Start {} => iface.start_save(),
                    SaveRequest::End {} => iface.end_save(),
                });

                ret_tx.send(result);
            }

            Ok(())
        };

        select! {
            _ = cancel.cancelled() => {}
            res = cmd_loop => {
                if res.is_err() {
                    return res;
                }
             }
        };

        Ok(())
    }
}
