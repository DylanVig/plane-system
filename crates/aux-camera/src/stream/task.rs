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
    interface: StreamInterface,
    cmd_rx: ChannelCommandSource<StreamRequest, StreamResponse>,
}

pub fn create_task(
    config: StreamConfig,
) -> anyhow::Result<(
    StreamTask,
    ChannelCommandSink<StreamRequest, StreamResponse>,
)> {
    let (cmd_tx, cmd_rx) = flume::bounded(256);

    let interface = StreamInterface::new(config.address, config.cameras)
        .context("failed to create stream interface")?;

    Ok((StreamTask { interface, cmd_rx }, cmd_tx))
}

#[async_trait]
impl Task for StreamTask {
    fn name(&self) -> &'static str {
        "aux-camera/stream"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            mut interface,
            cmd_rx,
        } = *self;

        let cmd_loop = async {
            trace!("initializing streamer");

            while let Ok((cmd, ret_tx)) = cmd_rx.recv_async().await {
                let result = tokio::task::block_in_place(|| match cmd {
                    StreamRequest::Start {} => interface.start_stream(),
                    StreamRequest::End {} => interface.end_stream(),
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
