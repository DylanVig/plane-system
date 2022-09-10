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

pub struct StreamTask {
    general_config: Config,
    stream_config: StreamConfig,
    cmd_rx: ChannelCommandSource<StreamRequest, StreamResponse>,
}

impl StreamTask {
    pub fn create(
        general_config: Config,
        stream_config: StreamConfig,
    ) -> (Self, ChannelCommandSink<StreamRequest, StreamResponse>) {
        let (cmd_tx, cmd_rx) = mpsc::channel(256);

        (
            Self {
                general_config,
                stream_config,
                cmd_rx,
            },
            cmd_tx,
        )
    }
}

#[async_trait]
impl Task for StreamTask {
    async fn run(self, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            general_config,
            stream_config,
            mut cmd_rx,
        } = self;

        let cmd_loop = async {
            let mut iface = StreamInterface::new(stream_config.address, general_config.cameras)
                .context("failed to create stream interface")?;

            trace!("initializing streamer");

            while let Some((cmd, ret_tx)) = cmd_rx.recv().await {
                let result = tokio::task::block_in_place(|| match cmd {
                    StreamRequest::Start {} => iface.start_stream(),
                    StreamRequest::End {} => iface.end_stream(),
                });

                let _ = ret_tx.send(result);
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
