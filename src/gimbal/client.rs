use anyhow::Context;

use futures::FutureExt;
use std::{path::Path, sync::Arc};

use crate::{Channels};

use super::{GimbalCommand, GimbalKind, GimbalRequest, GimbalResponse, interface::{GimbalInterface, HardwareGimbalInterface, SoftwareGimbalInterface}};

pub struct GimbalClient {
    iface: Box<dyn GimbalInterface + Send>,
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
            HardwareGimbalInterface::with_path(path).context("failed to create gimbal interface")?;

        Ok(Self {
            iface: Box::new(iface),
            channels,
            cmd,
        })
    }

    pub fn connect(
        channels: Arc<Channels>,
        cmd: flume::Receiver<GimbalCommand>,
        kind: GimbalKind,
    ) -> anyhow::Result<Self> {
        let iface: Box<dyn GimbalInterface + Send> = match kind {
            GimbalKind::Hardware { protocol } => Box::new(
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
            GimbalRequest::GPS { lat, lon } => {
                let telemetry_info = self.channels.telemetry.borrow().clone();

                if telemetry_info.is_none() {
                    warn!("no telemetry data available for gimbal pointing")
                }
                self.iface.point_at_gps(*lat, *lon, &telemetry_info).await?
            }
        }

        Ok(GimbalResponse::Unit)
    }
}
