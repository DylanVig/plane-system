use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::{state::Telemetry, Channels};

use std::{path::PathBuf, sync::Arc, time::Duration};

struct SchedulerState {
    active_rois: Vec<Roi>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Roi {
    id: usize,
    location: geo::Point<f32>,
    kind: RoiKind,

    // skip deserializing b/c we don't receive captures from outside
    #[serde(skip_deserializing)]
    captures: Vec<Capture>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoiKind {
    Normal,
    OffAxis,
    EmergentTarget,
}

/// Represents a capture of a certain ROI.
#[derive(Clone, Debug, Serialize)]
pub struct Capture {
    id: usize,

    /// IDs of ROIs which are present in this capture
    rois: Vec<usize>,

    timestamp: chrono::DateTime<chrono::Local>,

    telemetry: Telemetry,

    file: PathBuf,
}

#[derive(Debug)]
pub enum SchedulerCommand {
    AddROIs {
        rois: Vec<Roi>,
        tx: oneshot::Sender<()>,
    },
    GetROIs {
        tx: oneshot::Sender<Vec<Roi>>,
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
    let _telemetry_recv = channels.telemetry.clone();
    let interrupt_fut = interrupt_recv.recv();

    let loop_fut = async move {
        let mut state = SchedulerState {
            active_rois: vec![],
        };

        let mut interval = tokio::time::interval(Duration::from_millis(50));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    // if let Some(telemetry) = telemetry_recv.borrow().clone() {
                    //     // update the angle of the gimbal according to current
                    //     // telemetry information
                    //     run_update(&mut state, telemetry).await?;
                    // }
                }
                cmd = cmd_recv.recv_async() => {
                    run_command(&mut state, cmd?).await?;
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

async fn run_update(state: &mut SchedulerState, _telemetry: Telemetry) -> anyhow::Result<()> {
    // give all of the ROIs a priority
    state.active_rois.iter().map(|roi| {
        // prioritize ROIs that have not been photographed much
        let _rarity = 1. / roi.captures.len() as f32;
    });

    Ok(())
}

async fn run_command(_state: &mut SchedulerState, _cmd: SchedulerCommand) -> anyhow::Result<()> {
    Ok(())
}
