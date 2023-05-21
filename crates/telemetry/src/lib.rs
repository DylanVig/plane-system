use std::{collections::VecDeque, sync::Arc};

use async_trait::async_trait;
use chrono::prelude::*;
use futures::future::OptionFuture;
use log::info;
use ps_client::Task;
use ps_main_camera_csb::CsbEvent;
use ps_pixhawk::PixhawkEvent;
use ps_types::{Euler, Point3D, Velocity3D};
use serde::{Deserialize, Serialize};
use tokio::{
    select,
    sync::{watch, RwLock},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Telemetry {
    pub pixhawk: Option<PixhawkTelemetry>,
    pub csb: Option<CsbTelemetry>,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct PixhawkTelemetry {
    pub position: Option<(Point3D, DateTime<Local>)>,
    pub velocity: Option<(Velocity3D, DateTime<Local>)>,
    pub attitude: Option<(Euler, DateTime<Local>)>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CsbTelemetry {
    pub snapshot_before: PixhawkTelemetry,
    pub snapshot_after: PixhawkTelemetry,
    pub timestamp: DateTime<Local>,
}

pub struct TelemetryTask {
    pixhawk_evt_rx: Option<flume::Receiver<PixhawkEvent>>,
    csb_evt_rx: Option<flume::Receiver<ps_main_camera_csb::CsbEvent>>,
    telem_rx: watch::Receiver<Telemetry>,
    telem_tx: watch::Sender<Telemetry>,
}

pub fn create_task(
    pixhawk_evt_rx: Option<flume::Receiver<PixhawkEvent>>,
    csb_evt_rx: Option<flume::Receiver<ps_main_camera_csb::CsbEvent>>,
) -> anyhow::Result<TelemetryTask> {
    let (telem_tx, telem_rx) = watch::channel(Telemetry {
        pixhawk: None,
        csb: None,
    });

    Ok(TelemetryTask {
        pixhawk_evt_rx,
        csb_evt_rx,
        telem_rx,
        telem_tx,
    })
}

impl TelemetryTask {
    pub fn telemetry(&self) -> watch::Receiver<Telemetry> {
        self.telem_rx.clone()
    }
}

#[async_trait]
impl Task for TelemetryTask {
    fn name(&self) -> &'static str {
        "telemetry"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            pixhawk_evt_rx,
            csb_evt_rx,
            telem_tx,
            ..
        } = *self;

        let _ = telem_tx.send_modify(|t| t.pixhawk = Some(PixhawkTelemetry::default()));

        // list of previous telemetries received
        let mut pixhawk_history = VecDeque::with_capacity(20);

        loop {
            let pixhawk_recv_fut = pixhawk_evt_rx.as_ref().map(|chan| chan.recv_async());
            let csb_recv_fut = csb_evt_rx.as_ref().map(|chan| chan.recv_async());

            select! {
                _ = cancel.cancelled() => {
                    break;
                }

                evt = OptionFuture::from(pixhawk_recv_fut), if pixhawk_recv_fut.is_some() => {
                    // unwrap b/c if we are here, then the OptionFuture is OptionFuture(Some),
                    // so it will not evaluate to None when we await it

                    if let Ok(evt) = evt.unwrap() {
                        handle_pixhawk_event(evt, &mut pixhawk_history, &telem_tx);
                    }
                }

                evt = OptionFuture::from(csb_recv_fut), if csb_recv_fut.is_some() => {
                    // unwrap b/c if we are here, then the OptionFuture is OptionFuture(Some),
                    // so it will not evaluate to None when we await it

                    if let Ok(evt) = evt.unwrap() {
                        handle_csb_event(evt, &pixhawk_history, &telem_tx);
                    }
                }

                else => {
                    info!("no available telemetry sources, exiting");
                    break;
                }
            }
        }

        Ok(())
    }
}

fn handle_pixhawk_event(
    event: PixhawkEvent,
    pixhawk_history: &mut VecDeque<PixhawkTelemetry>,
    telem_tx: &watch::Sender<Telemetry>,
) {
    // base new telem on old telem, update fields
    let mut telem = pixhawk_history.back().cloned().unwrap_or_default();

    match event {
        PixhawkEvent::Gps {
            position, velocity, ..
        } => {
            let now = Local::now();
            telem.position = Some((position, now));
            telem.velocity = Some((velocity, now));
        }
        PixhawkEvent::Orientation { attitude } => {
            telem.attitude = Some((attitude, Local::now()));
        }
    }

    let _ = telem_tx.send_modify(|t| t.pixhawk = Some(telem.clone()));

    if pixhawk_history.len() >= 20 {
        pixhawk_history.pop_front();
    }

    pixhawk_history.push_back(telem);
}

fn handle_csb_event(
    event: CsbEvent,
    pixhawk_history: &VecDeque<PixhawkTelemetry>,
    telem_tx: &watch::Sender<Telemetry>,
) {
    let timestamp = event.timestamp;

    // get a snapshot of relevant pixhawk data by picking
    // the two closest data points for position,
    // orientation, and velocity
    let (before, after) = {
        let (position, velocity, attitude) = {
            // get latest telemetry information from before the csb timestamp
            let position = pixhawk_history
                .iter()
                .filter_map(|entry| {
                    let (position, position_ts) = entry.position?;
                    if position_ts < timestamp {
                        Some((position, position_ts))
                    } else {
                        None
                    }
                })
                .nth_back(0);

            let velocity = pixhawk_history
                .iter()
                .filter_map(|entry| {
                    let (velocity, velocity_ts) = entry.velocity?;
                    if velocity_ts < timestamp {
                        Some((velocity, velocity_ts))
                    } else {
                        None
                    }
                })
                .nth_back(0);

            let attitude = pixhawk_history
                .iter()
                .filter_map(|entry| {
                    let (attitude, attitude_ts) = entry.attitude?;
                    if attitude_ts < timestamp {
                        Some((attitude, attitude_ts))
                    } else {
                        None
                    }
                })
                .nth_back(0);

            (position, velocity, attitude)
        };

        let before = PixhawkTelemetry {
            position,
            velocity,
            attitude,
        };

        let (position, velocity, attitude) = {
            // get earliest telemetry information from after the csb timestamp
            let position = pixhawk_history
                .iter()
                .filter_map(|entry| {
                    let (position, position_ts) = entry.position?;
                    if position_ts > timestamp {
                        Some((position, position_ts))
                    } else {
                        None
                    }
                })
                .nth(0);

            let velocity = pixhawk_history
                .iter()
                .filter_map(|entry| {
                    let (velocity, velocity_ts) = entry.velocity?;
                    if velocity_ts > timestamp {
                        Some((velocity, velocity_ts))
                    } else {
                        None
                    }
                })
                .nth(0);

            let attitude = pixhawk_history
                .iter()
                .filter_map(|entry| {
                    let (attitude, attitude_ts) = entry.attitude?;
                    if attitude_ts > timestamp {
                        Some((attitude, attitude_ts))
                    } else {
                        None
                    }
                })
                .nth(0);

            (position, velocity, attitude)
        };

        let after = PixhawkTelemetry {
            position,
            velocity,
            attitude,
        };

        (before, after)
    };

    let _ = telem_tx.send_modify(|t| {
        t.csb = Some(CsbTelemetry {
            snapshot_before: before,
            snapshot_after: after,
            timestamp,
        })
    });
}
