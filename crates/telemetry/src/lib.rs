use async_trait::async_trait;
use chrono::prelude::*;
use ps_client::Task;
use ps_types::{Attitude, Point3D};
use tokio::{sync::watch, select};
use tokio_util::sync::CancellationToken;

pub struct Telemetry {
    pub location: (Point3D, DateTime<Local>),
    pub orientation: (Attitude, DateTime<Local>),
}

pub struct TelemetryTask {
    pixhawk_evt_rx: flume::Receiver<ps_pixhawk::PixhawkEvent>,
    telem_rx: watch::Receiver<Option<Telemetry>>,
    telem_tx: watch::Sender<Option<Telemetry>>,
}

pub fn create_task(pixhawk_task: &ps_pixhawk::EventTask) -> anyhow::Result<TelemetryTask> {
    let (telem_tx, telem_rx) = watch::channel(None);

    Ok(TelemetryTask {
        pixhawk_evt_rx: pixhawk_task.events(),
        telem_rx,
        telem_tx,
    })
}

impl TelemetryTask {
    pub fn telemetry(&self) -> watch::Receiver<Option<Telemetry>> {
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
            telem_tx,
            ..
        } = *self;

        let loop_fut = async move {
            let mut location = None;
            let mut orientation = None;

            loop {
                let evt = pixhawk_evt_rx.recv_async().await?;

                match evt {
                    ps_pixhawk::PixhawkEvent::Gps { position, .. } => {
                        location = Some((position, Local::now()));
                    },
                    ps_pixhawk::PixhawkEvent::Orientation { attitude } => {
                        orientation = Some((attitude, Local::now()));
                    },
                }

                if let (Some(location), Some(orientation)) = (location, orientation) {
                    let _ = telem_tx.send(Some(Telemetry {
                        location,
                        orientation
                    }));
                }
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
