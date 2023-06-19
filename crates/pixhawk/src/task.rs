use anyhow::bail;
use async_trait::async_trait;
use ps_client::Task;
use ps_types::{Euler, Point3D, Velocity3D};
use std::net::SocketAddr;
use tokio::select;
use tokio_util::sync::CancellationToken;
use uom::si::{angle::radian, f32::*, length::meter, velocity::meter_per_second};

use mavlink::{ardupilotmega as apm, common, MavlinkVersion};

use crate::{interface::PixhawkInterface, PixhawkConfig, PixhawkEvent};

pub fn create_tasks(config: PixhawkConfig) -> anyhow::Result<EventTask> {
    let (evt_tx, evt_rx) = flume::bounded(256);

    Ok(EventTask {
        address: config.address,
        version: match config.mavlink.as_str() {
            "V1" => MavlinkVersion::V1,
            "V2" => MavlinkVersion::V2,
            other => bail!("invalid mavlink version {other}"),
        },
        evt_tx,
        evt_rx,
    })
}

pub struct EventTask {
    address: SocketAddr,
    version: MavlinkVersion,
    evt_tx: flume::Sender<PixhawkEvent>,
    evt_rx: flume::Receiver<PixhawkEvent>,
}

impl EventTask {
    pub fn events(&self) -> flume::Receiver<PixhawkEvent> {
        self.evt_rx.clone()
    }
}

#[async_trait]
impl Task for EventTask {
    fn name(&self) -> &'static str {
        "pixhawk/event"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            evt_tx,
            address,
            version,
            ..
        } = *self;

        let loop_fut = async move {
            let mut interface = PixhawkInterface::connect(address, version).await?;

            interface.init().await?;

            loop {
                let message = interface.recv().await?;

                match message {
                    apm::MavMessage::GLOBAL_POSITION_INT(data) => {
                        let _ = evt_tx.send(PixhawkEvent::Gps {
                            position: Point3D {
                                point: geo::Point::new(
                                    data.lon as f32 / 1e7,
                                    data.lat as f32 / 1e7,
                                ),
                                altitude_msl: Length::new::<meter>(data.alt as f32 / 1e3),
                                altitude_rel: Length::new::<meter>(data.relative_alt as f32 / 1e3),
                            },
                            // velocity is provided as (North, East, Down)
                            // so we transform it to more common (East, North, Up)
                            velocity: Velocity3D::new::<meter_per_second>(
                                data.vy as f32 / 100.,
                                data.vx as f32 / 100.,
                                -data.vz as f32 / 100.,
                            ),
                        });
                    }
                    apm::MavMessage::ATTITUDE(data) => {
                        let _ = evt_tx.send(PixhawkEvent::Orientation {
                            attitude: Euler::new::<radian>(data.roll, data.pitch, data.yaw),
                        });
                    }
                    _ => {}
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
