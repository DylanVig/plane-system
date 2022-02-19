use anyhow::Context;

use std::path::PathBuf;
use std::sync::Arc;

use crate::util::run_loop;
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
            std::fs::create_dir(path).context("failed to create save directory")?;
        }

        Ok(Self {
            iface,
            channels,
            cmd,
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        trace!("initializing saver");

        let mut interrupt_rx = self.channels.interrupt.subscribe();

        run_loop!(
            async {
                while let Ok(cmd) = self.cmd.recv() {
                    let result = self.exec(cmd.request()).await;
                    let _ = cmd.respond(result);
                }

                Ok(())
            },
            interrupt_rx.recv()
        );

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
