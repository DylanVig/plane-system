use std::{sync::Arc, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use log::*;
use ps_client::Task;
use tokio::{select, sync::RwLock};
use tokio_util::sync::CancellationToken;

use super::InterfaceGuard;

pub struct EventTask {
    interface: Arc<RwLock<InterfaceGuard>>,
    evt_tx: flume::Sender<ptp::Event>,
    evt_rx: flume::Receiver<ptp::Event>,
}

impl EventTask {
    pub(super) fn new(interface: Arc<RwLock<InterfaceGuard>>) -> Self {
        let (evt_tx, evt_rx) = flume::bounded(256);

        Self {
            interface,
            evt_rx,
            evt_tx,
        }
    }

    pub fn events(&self) -> flume::Receiver<ptp::Event> {
        self.evt_rx.clone()
    }
}

#[async_trait]
impl Task for EventTask {
    fn name(&self) -> &'static str {
        "main-camera/event"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let loop_fut = async move {
            loop {
                let event = {
                    trace!("acquiring lock on interface");

                    let mut interface = self.interface.write().await;

                    trace!("checking for events on interface");

                    tokio::task::block_in_place(|| {
                        interface
                            .recv(Some(Duration::from_millis(100)))
                            .context("error while receiving camera event")
                    })?
                };

                if let Some(event) = event {
                    debug!("recv event {:?}", event);

                    if let Err(_) = self.evt_tx.send_async(event).await {
                        warn!("failed to publish event, exiting");
                        break;
                    }
                }

                tokio::task::yield_now().await;
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
