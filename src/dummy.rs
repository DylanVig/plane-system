use std::{time::Duration, sync::Arc};

use futures::{FutureExt, StreamExt};
use serde::Serialize;
use structopt::StructOpt;

use crate::{Channels, Command};

pub type DummyCommand = Command<DummyRequest, DummyResponse>;

#[derive(StructOpt, Debug, Clone)]
pub enum DummyRequest {
    Dummy,
}

#[derive(Debug, Clone, Serialize)]
pub enum DummyResponse {
    Unit,
}

pub struct DummyClient {
    channels: Arc<Channels>,
    cmd: flume::Receiver<DummyCommand>,
}

impl DummyClient {
    pub fn create(
        channels: Arc<Channels>,
        cmd: flume::Receiver<DummyCommand>,
    ) -> anyhow::Result<Self> {
        Ok(DummyClient { channels, cmd })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut interrupt_recv = self.channels.interrupt.subscribe();
        let interrupt_fut = interrupt_recv.recv().fuse();
        futures::pin_mut!(interrupt_fut);

        let telemetry_chan = self.channels.telemetry.clone();
        let mut telemetry_stream = tokio_stream::wrappers::WatchStream::new(telemetry_chan).fuse();
        let mut cmd_stream = self.cmd.clone().into_stream().fuse();
        
        loop {
            futures::select! {
                cmd = cmd_stream.next() => {
                    // this is only None if the command stream closes for some reason
                    let cmd = cmd.unwrap();
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    let result = Ok(DummyResponse::Unit);
                    trace!("command completed, sending response");
                    cmd.respond(result).expect("help");
                }
                _telemetry = telemetry_stream.next() => {
                }
                _ = &mut interrupt_fut => break,
            }
        }

        Ok(())
    }
}
