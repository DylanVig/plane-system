use std::{sync::Arc, time::Duration};

use anyhow::Context;
use tokio::{
    sync::broadcast::{self, TryRecvError},
    task::spawn_blocking,
};

use crate::{
    cli::repl::{CameraCliCommand, CliCommand},
    Channels,
};

use super::{interface::CameraInterface, state::CameraMessage};

pub struct CameraClient {
    iface: CameraInterface,
    channels: Arc<Channels>,
    cli: broadcast::Receiver<CliCommand>,
    interrupt: broadcast::Receiver<()>,
}

impl CameraClient {
    pub fn connect(channels: Arc<Channels>) -> anyhow::Result<Self> {
        let iface = CameraInterface::new().context("failed to create camera interface")?;

        let cli = channels.cli.subscribe();
        let interrupt = channels.interrupt.subscribe();

        Ok(CameraClient {
            iface,
            channels,
            cli,
            interrupt,
        })
    }

    pub fn init(&mut self) -> anyhow::Result<()> {
        info!("intializing camera");

        self.iface.connect()?;

        info!("initialized camera");

        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.init()?;

        loop {
            match self.interrupt.try_recv() {
                Ok(_) => break,
                Err(TryRecvError::Empty) => {}
                Err(_) => todo!("handle interrupt receiver lagging??"),
            }

            match self.cli.try_recv() {
                Ok(CliCommand::Camera(cmd)) => match cmd {
                    CameraCliCommand::ChangeDirectory { directory } => {}
                    CameraCliCommand::EnumerateDirectory { deep } => {
                        self.iface.storage_ids()
                    }
                    CameraCliCommand::Capture => {}
                    CameraCliCommand::Zoom { level } => {}
                    CameraCliCommand::Download { file } => {}
                },
                Ok(CliCommand::Exit) => break,
                Ok(_) | Err(_) => {}
            }

            tokio::time::delay_for(Duration::from_secs(1)).await;
        }

        Ok(())
    }
}
