use crate::Config;
use anyhow::Context;
use async_trait::async_trait;
use log::*;
use ps_client::{ChannelCommandSink, ChannelCommandSource, Task};
use tokio::select;
use tokio_util::sync::CancellationToken;

use super::interface::*;
use super::*;

pub struct StreamTask {
    general_config: Config,
    stream_config: StreamConfig,
    cmd_rx: ChannelCommandSource<StreamRequest, StreamResponse>,
}

pub fn create_task(
    general_config: Config,
    stream_config: StreamConfig,
) -> (StreamTask, ChannelCommandSink<StreamRequest, StreamResponse>) {
    let (cmd_tx, cmd_rx) = flume::bounded(256);

    (
        StreamTask {
            general_config,
            stream_config,
            cmd_rx,
        },
        cmd_tx,
    )
}

#[async_trait]
impl Task for StreamTask {
    fn name() -> &'static str {
        "aux-camera/stream"
    }

    async fn run(self, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            general_config,
            stream_config,
            cmd_rx,
        } = self;

        let cmd_loop = async {
            let mut iface = StreamInterface::new(stream_config.address, general_config.cameras)
                .context("failed to create stream interface")?;

            trace!("initializing streamer");

            while let Ok((cmd, ret_tx)) = cmd_rx.recv_async().await {
                let result = tokio::task::block_in_place(|| match cmd {
                    StreamRequest::Start {} => iface.start_stream(),
                    StreamRequest::End {} => iface.end_stream(),
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
