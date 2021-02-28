use anyhow::Context;
use futures::FutureExt;
use std::{path::Path, sync::Arc};

use crate::Channels;

use super::interface::*;
use super::*;

pub struct GimbalClient {
    iface: GimbalInterface,
    channels: Arc<Channels>,
    cmd: flume::Receiver<GimbalCommand>,
}

impl GimbalClient {
    pub fn connect_with_path<P: AsRef<Path>>(
        channels: Arc<Channels>,
        cmd: flume::Receiver<GimbalCommand>,
        path: P,
    ) -> anyhow::Result<Self> {
        let iface =
            GimbalInterface::with_path(path).context("failed to create gimbal interface")?;

        Ok(Self {
            iface,
            channels,
            cmd,
        })
    }

    pub fn connect(
        channels: Arc<Channels>,
        cmd: flume::Receiver<GimbalCommand>,
    ) -> anyhow::Result<Self> {
        let iface = GimbalInterface::new().context("failed to create gimbal interface")?;

        Ok(Self {
            iface,
            channels,
            cmd,
        })
    }

    pub fn init(&self) -> anyhow::Result<()> {
        trace!("initializing gimbal");
        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.init()?;

        let mut interrupt_recv = self.channels.interrupt.subscribe();
        let interrupt_fut = interrupt_recv.recv().fuse();
        futures::pin_mut!(interrupt_fut);

        loop {
            futures::select! {
                cmd = self.cmd.recv_async().fuse() => {
                    if let Ok(cmd) = cmd {
                        let result = self.exec(cmd.request()).await;
                        let _ = cmd.respond(result);
                    }
                }
                _ = interrupt_fut => break,
            }
        }
        
        Ok(())
    }

    async fn exec(&mut self, cmd: &GimbalRequest) -> anyhow::Result<GimbalResponse> {
        match cmd {
            GimbalRequest::Control { roll, pitch } => {
                self.iface.control_angles(*roll, *pitch).await?
            }
        }
        Ok(GimbalResponse::Unit)
    }
}
