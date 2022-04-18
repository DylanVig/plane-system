use geo::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::{state::Telemetry, Channels};

use std::{cmp::Ordering, path::PathBuf, sync::Arc, time::Duration};

struct SchedulerState {
    /// The ROIs that are still in the running for being captured.
    active_rois: Vec<Roi>,

    /// The ROI that is currently being focused on by the camera.
    target_roi: Option<TargetRoi>,
}

struct TargetRoi {
    roi: Roi,

    /// The closest distance at which this ROI has been seen. Used to figure out
    /// when the distance to the ROI starts increasing, so that we can capture a
    /// photo.
    closest_distance: f32,
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
    let telemetry_recv = channels.telemetry.clone();
    let interrupt_fut = interrupt_recv.recv();

    let loop_fut = async move {
        let mut state = SchedulerState {
            active_rois: vec![],
            target_roi: None,
        };

        let mut interval = tokio::time::interval(Duration::from_millis(50));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let telem = telemetry_recv.borrow().clone();
                    if let Some(telemetry) = telem {
                        // update the angle of the gimbal according to current
                        // telemetry information
                        run_update(&mut state, telemetry).await?;
                    }
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

async fn run_update(state: &mut SchedulerState, telemetry: Telemetry) -> anyhow::Result<()> {
    let should_update_target_roi = match &mut state.target_roi {
        Some(target_roi) => {
            let target_roi_distance = f32::sqrt(
                f32::powi(
                    target_roi
                        .roi
                        .location
                        .haversine_distance(&telemetry.position.point),
                    2,
                ) + f32::powi(telemetry.position.altitude_rel, 2),
            );

            // if we're getting farther away from the ROI, then assume that we
            // are currently as close as we're going to get, and take the photo
            if target_roi_distance < target_roi.closest_distance {
                target_roi.closest_distance = target_roi_distance;
                false
            } else {
                // issue camera shutter
                true
            }
        }
        None => true,
    };

    if should_update_target_roi {
        if state.active_rois.len() > 0 {
            // give all of the ROIs a priority
            let mut scored_rois: Vec<_> = state
                .active_rois
                .iter()
                .map(|roi| {
                    // prioritize ROIs that have not been photographed much
                    let rarity = 1. / (1. + roi.captures.len() as f32);
                    let distance = f32::sqrt(
                        f32::powi(
                            roi.location.haversine_distance(&telemetry.position.point),
                            2,
                        ) + f32::powi(telemetry.position.altitude_rel, 2),
                    );
                    let proximity = f32::exp(-distance / 100.);
                    let score = rarity + proximity;

                    (roi, distance, score)
                })
                .collect();

            // sort the ROIs by priority
            scored_rois.sort_by(|(_, _, a), (_, _, b)| a.partial_cmp(b).unwrap_or(Ordering::Less));

            let &(next_roi, next_roi_distance, next_roi_score) = scored_rois.first().unwrap();

            state.target_roi = Some(TargetRoi {
                roi: next_roi.clone(),
                closest_distance: next_roi_distance,
            });
        } else {
            state.target_roi = None;
        }
    }

    Ok(())
}

async fn run_command(state: &mut SchedulerState, cmd: SchedulerCommand) -> anyhow::Result<()> {
    match cmd {
        SchedulerCommand::AddROIs { rois, tx } => {
            state.active_rois.extend(rois);
            let _ = tx.send(());
        }
        SchedulerCommand::GetROIs { tx } => {
            let _ = tx.send(state.active_rois.clone());
        }
        SchedulerCommand::GetCaptures { tx } => todo!(),
    }

    Ok(())
}
