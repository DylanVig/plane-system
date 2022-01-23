use anyhow::Context;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::Channels;


use super::interface::*;
use super::*;

pub struct SaveClient {
    iface: SaveInterface,
    channels: Arc<Channels>,
    cmd: flume::Receiver<SaveCommand>,
}

impl SaveClient {
    pub fn connect(
        channels: Arc<Channels>,
        cmd: flume::Receiver<SaveCommand>,
        path: PathBuf,
        cameras: Vec<String>,
    ) -> anyhow::Result<Self> {
        let iface =
            SaveInterface::new(path.clone(), cameras).context("failed to create save interface")?;
        if !path.exists() {
            std::fs::create_dir(path).unwrap_or_else(|e| panic!("Error creating dir: {}", e));
        }
        Ok(Self {
            iface,
            channels,
            cmd,
        })
    }

    pub fn init(&self) -> anyhow::Result<()> {
        trace!("initializing saver");
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

    async fn exec(&mut self, cmd: &SaveRequest) -> anyhow::Result<SaveResponse> {
        match cmd {
            SaveRequest::Start {} => self.iface.start_save()?,
            SaveRequest::End {} => self.iface.end_save()?,
        }
        Ok(SaveResponse::Unit)
    }
}
