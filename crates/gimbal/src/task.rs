use anyhow::{bail, Context};
use async_trait::async_trait;
use futures::FutureExt;
use tokio_util::sync::CancellationToken;
use tracing::log::*;

use crate::{
    config::GimbalConfig,
    interface::{GimbalInterface, HardwareGimbalInterface, SoftwareGimbalInterface},
    GimbalCommand, GimbalKind, GimbalRequest, GimbalResponse,
};

pub struct GimbalTask {
    iface: Box<dyn GimbalInterface + Send>,
    cmd_tx: flume::Sender<GimbalCommand>,
    cmd_rx: flume::Receiver<GimbalCommand>,
}

impl GimbalTask {
    pub fn connect_with_path<P: AsRef<str>>(
        cmd_tx: flume::Sender<GimbalCommand>,
        cmd_rx: flume::Receiver<GimbalCommand>,
        path: P,
    ) -> anyhow::Result<Self> {
        let iface = HardwareGimbalInterface::with_path(path)
            .context("failed to create gimbal interface")?;

        Ok(Self {
            iface: Box::new(iface),
            cmd_tx,
            cmd_rx,
        })
    }

    pub fn connect(
        cmd_tx: flume::Sender<GimbalCommand>,
        cmd_rx: flume::Receiver<GimbalCommand>,
        kind: GimbalKind,
    ) -> anyhow::Result<Self> {
        let iface: Box<dyn GimbalInterface + Send> = match kind {
            GimbalKind::Hardware { protocol: _ } => Box::new(
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
            cmd_tx,
            cmd_rx,
        })
    }

    pub fn cmd(&self) -> flume::Sender<GimbalCommand> {
        self.cmd_tx.clone()
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

#[async_trait]
impl ps_client::Task for GimbalTask {
    fn name(&self) -> &'static str {
        "gimbal"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            mut iface, cmd_rx, ..
        } = *self;

        let loop_fut = async move {
            while let Ok((cmd, return_chan)) = cmd_rx.recv_async().await {
                trace!("cmd = {cmd:?}");

                match cmd {
                    GimbalRequest::Control { roll, pitch } => {
                        let result = iface.control_angles(roll, pitch).await;
                        trace!("result = {result:?}");
                        let _ = return_chan.send(result.map(|_| GimbalResponse::Unit));
                    }
                }
            }

            trace!("🤖");

            Ok::<_, anyhow::Error>(())
        };

        tokio::select! {
          _ = cancel.cancelled() => {}
          res = loop_fut => { res? }
        }

        Ok(())
    }
}

pub fn create_task(config: GimbalConfig) -> anyhow::Result<GimbalTask> {
    let (cmd_tx, cmd_rx) = flume::bounded(256);

    if let Some(path) = config.path {
        match config.kind {
            GimbalKind::Hardware { .. } => GimbalTask::connect_with_path(cmd_tx, cmd_rx, path),
            GimbalKind::Software => {
                bail!("supplying gimbal device path is not supported for software gimbal")
            }
        }
    } else {
        GimbalTask::connect(cmd_tx, cmd_rx, config.kind)
    }
}
