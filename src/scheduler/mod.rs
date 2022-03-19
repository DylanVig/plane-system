use anyhow::Context;
use tokio::sync::oneshot;

use crate::{
    gimbal::GimbalRequest,
    state::{Coords2D, RegionOfInterest, RegionOfInterestId, TelemetryInfo},
    Channels, Command,
};

use std::{path::PathBuf, sync::Arc, time::Duration};

mod backend;
mod state;

use backend::*;

#[derive(Clone, Debug)]
/// Represents a capture of a certain ROI.
pub struct Capture {
    id: usize,

    /// IDs of ROIs which are present in this capture
    rois: Vec<RegionOfInterestId>,

    timestamp: chrono::DateTime<chrono::Local>,

    telemetry: TelemetryInfo,

    file: PathBuf,
}

#[derive(Debug)]
pub enum SchedulerCommand {
    AddROIs {
        rois: Vec<RegionOfInterest>,
        tx: oneshot::Sender<()>,
    },
    GetROIs {
        tx: oneshot::Sender<Vec<RegionOfInterest>>,
    },
    GetCaptures {
        tx: oneshot::Sender<Vec<Capture>>,
    },
}

pub async fn run(
    channels: Arc<Channels>,
    cmd_recv: flume::Receiver<SchedulerCommand>,
) -> anyhow::Result<()> {
    let mut interrupt_recv = channels.interrupt.subscribe();
    let interrupt_fut = interrupt_recv.recv();

    let loop_fut = async move {
        let mut interval = tokio::time::interval(Duration::from_millis(50));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                            _ = interval.tick() => {
                                // update the angle of the gimbal according to current
                                // telemetry information
                            }
                            cmd = cmd_recv.recv_async() => {
                                match cmd? {
                SchedulerCommand::AddROIs { rois, tx } => todo!(),
                SchedulerCommand::GetROIs { tx } => todo!(),
                SchedulerCommand::GetCaptures { tx } => todo!(),
            }
                            }
                        };
        }

        // this is necessary so that Rust can figure out what the return
        // type of the async block is
        #[allow(unreachable_code)]
        Result::<(), anyhow::Error>::Ok(())
    };

    futures::pin_mut!(loop_fut);
    futures::pin_mut!(interrupt_fut);
    futures::future::select(interrupt_fut, loop_fut).await;

    Ok(())
}
