use anyhow::Context;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::Channels;

use super::interface::*;
use super::*;

pub struct GimbalClient {
    iface: GimbalInterface,
    channels: Arc<Channels>,
    cmd: mpsc::Receiver<GimbalCommand>,
}

impl GimbalClient {
    pub fn connect(
        channels: Arc<Channels>,
        cmd: mpsc::Receiver<GimbalCommand>,
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

        loop {
            if let Some(cmd) = self.cmd.recv().await {
                let result = self.exec(cmd.request()).await;
                let _ = cmd.respond(result);
            }

            if interrupt_recv.try_recv().is_ok() {
                break;
            }

            tokio::time::sleep(Duration::from_millis(10)).await;
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
