use anyhow::Context;
use std::sync::Arc;
use std::time::Duration;

use crate::Channels;
use tokio::sync::mpsc;

use super::interface::*;
use super::*;

pub struct StreamClient {
  iface: StreamInterface,
  channels: Arc<Channels>,
  cmd: mpsc::Receiver<StreamCommand>,
}

impl StreamClient {
  pub fn connect(
    channels: Arc<Channels>,
    cmd: mpsc::Receiver<StreamCommand>,
    mode: bool,
    address: String,
  ) -> anyhow::Result<Self> {
    let iface = StreamInterface::new(mode, address).context("failed to create stream interface")?;

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

  async fn exec(&mut self, cmd: &StreamRequest) -> anyhow::Result<StreamResponse> {
    match cmd {
      StreamRequest::Start {} => self.iface.start_stream()?,
      StreamRequest::End {} => self.iface.end_stream()?,
    }
    Ok(StreamResponse::Unit)
  }
}
