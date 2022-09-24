use anyhow::Context;
use log::*;

use async_trait::async_trait;
use ps_client::Task;
use tokio::select;
use tokio_util::sync::CancellationToken;

use crate::{PixhawkEvent, PixhawkConfig};

pub struct EventTask {
  evt_tx: flume::Sender<PixhawkEvent>,
  evt_rx: flume::Receiver<PixhawkEvent>,
}

pub fn create_task(config: PixhawkConfig) -> anyhow::Result<EventTask> {
    todo!()
}

#[async_trait]
impl Task for EventTask {
    fn name(&self) -> &'static str {
        "pixhawk/event"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            ..
        } = *self;

        let loop_fut = async move {
            loop {
              todo!()
            }

            #[allow(unreachable_code)]
            Ok::<_, anyhow::Error>(())
        };

        select! {
          _ = cancel.cancelled() => {}
          res = loop_fut => { res? }
        }

        Ok(())
    }
}
