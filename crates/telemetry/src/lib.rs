use async_trait::async_trait;
use chrono::prelude::*;
use ps_client::Task;
use ps_types::{Attitude, Point3D};
use tokio::{select, sync::watch};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct Telemetry {
    pub pixhawk: Option<PixhawkTelemetry>,
    pub csb: Option<CsbTelemetry>,
}

#[derive(Clone)]
pub struct PixhawkTelemetry {
    pub position: (Point3D, DateTime<Local>),
    pub attitude: (Attitude, DateTime<Local>),
}

#[derive(Clone)]
pub struct CsbTelemetry {
    pub position: Point3D,
    pub attitude: Attitude,
    pub timestamp: DateTime<Local>,
}

pub struct TelemetryTask {
    pixhawk_evt_rx: Option<flume::Receiver<ps_pixhawk::PixhawkEvent>>,
    csb_evt_rx: Option<flume::Receiver<ps_main_camera_csb::CsbEvent>>,
    telem_rx: watch::Receiver<Telemetry>,
    telem_tx: watch::Sender<Telemetry>,
}

pub fn create_task(
    pixhawk_evt_rx: Option<flume::Receiver<ps_pixhawk::PixhawkEvent>>,
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

        let loop_fut = async move {
            // store position and attitude, because pixhawk sends these
            // separately, but we only want to publish pixhawk telem once we
            // have both
            let mut pixhawk_position = None;
            let mut pixhawk_attitude = None;

            loop {
                let pixhawk_recv_fut = pixhawk_evt_rx.as_ref().map(|chan| chan.recv_async());
                let csb_recv_fut = csb_evt_rx.as_ref().map(|chan| chan.recv_async());

                select! {
                    evt = pixhawk_recv_fut.unwrap(), if pixhawk_recv_fut.is_some() => {
                        match evt? {
                            ps_pixhawk::PixhawkEvent::Gps { position, .. } => {
                                pixhawk_position = Some((position, Local::now()));
                            },
                            ps_pixhawk::PixhawkEvent::Orientation { attitude } => {
                                pixhawk_attitude = Some((attitude, Local::now()));
                            },
                        }


                        if let (Some(position), Some(attitude)) = (pixhawk_position, pixhawk_attitude) {
                            let _ = telem_tx
                                .send_modify(|t| t.pixhawk = Some(PixhawkTelemetry { position, attitude }));
                        }
                    }

                    evt = csb_recv_fut.unwrap(), if csb_recv_fut.is_some() => {
                        let evt = evt?;

                        let _ = telem_tx
                            .send_modify(|t| t.csb = Some(CsbTelemetry {
                                position: Default::default(),
                                attitude: Default::default(),
                                timestamp: evt.timestamp,
                            }));
                    }
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
