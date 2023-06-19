use std::{collections::VecDeque, sync::Arc};

use async_trait::async_trait;
use chrono::{prelude::*, Duration};
use futures::future::OptionFuture;
use log::info;
use ps_client::Task;
#[cfg(feature = "csb")]
use ps_main_camera_csb::CsbEvent;
use ps_pixhawk::PixhawkEvent;
use ps_types::{Euler, Point3D, Velocity3D};
use serde::{Deserialize, Serialize};
use tokio::{
    select,
    sync::{watch, RwLock},
};
use tokio_util::sync::CancellationToken;

mod config;

pub use config::TelemetryConfig;

#[derive(Clone, Serialize, Debug)]
pub struct Telemetry {
    /// Contains Pixhawk telemetries from oldest to newest
    pub pixhawk: VecDeque<PixhawkTelemetry>,
    /// Contains information from the most recent current sensing board spike
    #[cfg(feature = "csb")]
    pub csb: Option<CsbTelemetry>,
}

#[derive(Clone, Serialize, Debug, Default)]
pub struct PixhawkTelemetry {
    pub position: Option<(Point3D, DateTime<Local>)>,
    pub velocity: Option<(Velocity3D, DateTime<Local>)>,
    pub attitude: Option<(Euler, DateTime<Local>)>,
    pub timestamp: DateTime<Local>,
}

#[derive(Clone, Serialize, Debug)]
pub struct CsbTelemetry {
    pub timestamp: DateTime<Local>,
}

pub struct TelemetryTask {
    config: TelemetryConfig,
    pixhawk_evt_rx: Option<flume::Receiver<PixhawkEvent>>,
    csb_evt_rx: Option<flume::Receiver<CsbEvent>>,
    telem_rx: watch::Receiver<Telemetry>,
    telem_tx: watch::Sender<Telemetry>,
}

pub fn create_task(
    config: TelemetryConfig,
    pixhawk_evt_rx: Option<flume::Receiver<PixhawkEvent>>,
    csb_evt_rx: Option<flume::Receiver<CsbEvent>>,
) -> anyhow::Result<TelemetryTask> {
    let (telem_tx, telem_rx) = watch::channel(Telemetry {
        pixhawk: VecDeque::new(),
        #[cfg(feature = "csb")]
        csb: None,
    });

    Ok(TelemetryTask {
        config,
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
            config,
            pixhawk_evt_rx,
            csb_evt_rx,
            telem_tx,
            ..
        } = *self;

        let retention_period = Duration::milliseconds((config.retention_period * 1000.0) as i64);

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
                        handle_pixhawk_event(evt, &telem_tx, retention_period);
                    }
                }

                evt = OptionFuture::from(csb_recv_fut), if csb_recv_fut.is_some() => {
                    // unwrap b/c if we are here, then the OptionFuture is OptionFuture(Some),
                    // so it will not evaluate to None when we await it

                    if let Ok(evt) = evt.unwrap() {
                        handle_csb_event(evt, &telem_tx);
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
    telem_tx: &watch::Sender<Telemetry>,
    retention_period: Duration,
) {
    let _ = telem_tx.send_modify(|t| {
        let pixhawk_history = &mut t.pixhawk;

        // base new telem on old telem, update fields
        let mut telem = pixhawk_history.back().cloned().unwrap_or_default();

        let now = Local::now();

        match event {
            PixhawkEvent::Gps {
                position, velocity, ..
            } => {
                telem.position = Some((position, now));
                telem.velocity = Some((velocity, now));
            }
            PixhawkEvent::Orientation { attitude } => {
                telem.attitude = Some((attitude, now));
            }
        }

        telem.timestamp = now;

        let threshold = now - retention_period;

        while let Some(first) = pixhawk_history.front() {
            if first.timestamp < threshold {
                pixhawk_history.pop_front();
            }
        }

        pixhawk_history.push_back(telem);
    });
}

fn handle_csb_event(event: CsbEvent, telem_tx: &watch::Sender<Telemetry>) {
    let _ = telem_tx.send_modify(|t| {
        t.csb = Some(CsbTelemetry {
            timestamp: event.timestamp,
        })
    });
}
