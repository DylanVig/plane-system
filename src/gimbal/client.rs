use anyhow::Context;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::Channels;

use super::{
    interface::{GimbalInterface, GimbalKind, HardwareGimbalInterface, SoftwareGimbalInterface},
    GimbalCommand, GimbalRequest, GimbalResponse,
};

pub struct GimbalClient {
    iface: Box<dyn GimbalInterface + Send>,
    channels: Arc<Channels>,
    cmd: mpsc::Receiver<GimbalCommand>,
}

impl GimbalClient {
    /// Connects to a physical hardware gimbal.
    pub fn connect(
        channels: Arc<Channels>,
        cmd: mpsc::Receiver<GimbalCommand>,
        kind: GimbalKind,
    ) -> anyhow::Result<Self> {
        let iface: Box<dyn GimbalInterface + Send> = match kind {
            GimbalKind::Hardware => Box::new(
                HardwareGimbalInterface::new()
                    .context("failed to create hardware gimbal interface")?,
            ),
            GimbalKind::Software => Box::new(
                SoftwareGimbalInterface::new()
                    .context("failed to create software gimbal interface")?,
            ),
        };

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
            if let Ok(cmd) = self.cmd.try_recv() {
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
            GimbalRequest::Control { roll, pitch } => self.iface.control_angles(*roll, *pitch)?,
        }

        Ok(GimbalResponse::Unit)
    }
}
