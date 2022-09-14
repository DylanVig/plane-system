use std::{sync::Arc, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use log::*;
use ps_client::Task;
use ptp::PtpEvent;
use tokio::{
    select,
    sync::{mpsc, RwLock},
};
use tokio_util::sync::CancellationToken;

use crate::interface::CameraInterface;


pub struct EventTask {
  interface: Arc<RwLock<CameraInterface>>,
  evt_tx: mpsc::Sender<PtpEvent>,
}

#[async_trait]
impl Task for EventTask {
  fn name() -> &'static str {
      "main-camera/event"
  }

  async fn run(self, cancel: CancellationToken) -> anyhow::Result<()> {
      let loop_fut = async move {
          loop {
              let event = {
                  self.interface
                      .read()
                      .await
                      .recv(Some(Duration::from_millis(100)))
                      .context("error while receiving camera event")?
              };

              if let Some(event) = event {
                  debug!("recv event {:?}", event);

                  if let Err(_) = self.evt_tx.send(event).await {
                      warn!("failed to publish event, exiting");
                      break;
                  }
              }
          }

          Ok::<_, anyhow::Error>(())
      };

      select! {
        _ = cancel.cancelled() => {}
        res = loop_fut => { res? }
      }

      Ok(())
  }
}
