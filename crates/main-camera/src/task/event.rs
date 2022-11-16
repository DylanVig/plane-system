use std::{sync::Arc, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use log::*;
use ps_client::Task;
use tokio::{
    select,
    sync::{broadcast, RwLock},
};
use tokio_util::sync::CancellationToken;
use tracing::trace_span;

use super::InterfaceGuard;

pub struct EventTask {
    interface: Arc<RwLock<InterfaceGuard>>,
    evt_tx: broadcast::Sender<ptp::Event>,
}

impl EventTask {
    pub(super) fn new(interface: Arc<RwLock<InterfaceGuard>>) -> Self {
        let (evt_tx, _) = broadcast::channel(256);

        Self { interface, evt_tx }
    }

    pub fn events(&self) -> broadcast::Receiver<ptp::Event> {
        self.evt_tx.subscribe()
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
                    let mut interface = self.interface.write().await;

                    let _enter = trace_span!("checking for events on interface").entered();

                    tokio::task::block_in_place(|| {
                        interface
                            .recv(Some(Duration::from_millis(100)))
                            .context("error while receiving camera event")
                    })?
                };

                if let Some(event) = event {
                    debug!("recv event {:?}", event);

                    if let Err(_) = self.evt_tx.send(event) {
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
