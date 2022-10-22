use std::net::SocketAddr;

use anyhow::Context;
use log::*;

use async_trait::async_trait;
use ps_client::Task;
use ps_types::{Attitude, Point3D};
use tokio::select;
use tokio_util::sync::CancellationToken;

use mavlink::{ardupilotmega as apm, common, MavlinkVersion};

use crate::{interface::PixhawkInterface, PixhawkConfig, PixhawkEvent};

pub struct EventTask {
    address: SocketAddr,
    version: MavlinkVersion,
    evt_tx: flume::Sender<PixhawkEvent>,
    evt_rx: flume::Receiver<PixhawkEvent>,
}

pub fn create_task(config: PixhawkConfig) -> anyhow::Result<EventTask> {
    let (evt_tx, evt_rx) = flume::bounded(256);

    Ok(EventTask {
        address: config.address,
        version: config.mavlink,
        evt_tx,
        evt_rx,
    })
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

            loop {
                let message = interface.recv().await?;

                match message {
                    apm::MavMessage::common(common::MavMessage::GLOBAL_POSITION_INT(data)) => {
                        let _ = evt_tx.send(PixhawkEvent::Gps {
                            position: Point3D {
                                point: geo::Point::new(
                                    data.lon as f32 / 1e7,
                                    data.lat as f32 / 1e7,
                                ),
                                altitude_msl: data.alt as f32 / 1e3,
                                altitude_rel: data.relative_alt as f32 / 1e3,
                            },
                            // velocity is provided as (North, East, Down)
                            // so we transform it to more common (East, North, Up)
                            velocity: (
                                data.vy as f32 / 100.,
                                data.vx as f32 / 100.,
                                -data.vz as f32 / 100.,
                            ),
                        });
                    }
                    apm::MavMessage::common(common::MavMessage::ATTITUDE(data)) => {
                        let _ = evt_tx.send(PixhawkEvent::Orientation {
                            attitude: Attitude::new(
                                data.roll.to_degrees(),
                                data.pitch.to_degrees(),
                                data.yaw.to_degrees(),
                            ),
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
