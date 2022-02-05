use anyhow::Context;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::util::{run_loop};
use crate::Channels;

use super::interface::*;
use super::*;

pub struct StreamClient {
    iface: StreamInterface,
    channels: Arc<Channels>,
    cmd: flume::Receiver<StreamCommand>,
}

impl StreamClient {
    pub fn connect(
        channels: Arc<Channels>,
        cmd: flume::Receiver<StreamCommand>,
        address: SocketAddr,
        cameras: Vec<String>,
    ) -> anyhow::Result<Self> {
        let iface =
            StreamInterface::new(address, cameras).context("failed to create stream interface")?;

        Ok(Self {
            iface,
            channels,
            cmd,
        })
    }

    pub fn init(&self) -> anyhow::Result<()> {
        trace!("initializing stream");
        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.init()?;

        let mut interrupt_recv = self.channels.interrupt.subscribe();

        run_loop!(
            async move {
                while let Ok(cmd) = self.cmd.recv() {
                    let result = self.exec(cmd.request()).await;
                    let _ = cmd.respond(result);
                }

                Ok(())
            },
            interrupt_recv.recv()
        );

        Ok(())
    }

    async fn exec(&mut self, cmd: &StreamRequest) -> anyhow::Result<StreamResponse> {
        match cmd {
            StreamRequest::Start {} => self.iface.start_stream()?,
            StreamRequest::End {} => self.iface.end_stream()?,
        }
        Ok(StreamResponse::Unit)
    }
}
