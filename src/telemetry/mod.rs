use crate::{pixhawk::state::PixhawkEvent, state::TelemetryInfo, util::ReceiverExt, Channels};

use std::sync::{Arc, Mutex};

use anyhow::Context;
use std::time::Duration;
use tokio::time::interval;
use tokio::{spawn, sync::watch};

// Noteworthy that this isn't a RwLock because we have at most one reader at any given moment
type TelemetryState = Arc<Mutex<TelemetryInfo>>;

struct TelemetryCollector {
    state: TelemetryState,
    channels: Arc<Channels>,
}

struct TelemetryPublisher {
    state: TelemetryState,
    sender: watch::Sender<Option<TelemetryInfo>>,
    channels: Arc<Channels>,
}

pub struct TelemetryStream {
    collector: TelemetryCollector,
    publisher: TelemetryPublisher,
}

impl TelemetryCollector {
    fn new(telemetry_state: TelemetryState, channels: Arc<Channels>) -> Self {
        Self {
            state: telemetry_state,
            channels,
        }
    }

    async fn run(&self) -> anyhow::Result<()> {
        let mut pixhawk_recv = self.channels.pixhawk_event.subscribe();
        let mut interrupt_recv = self.channels.interrupt.subscribe();

        loop {
            let message = pixhawk_recv
                .recv_skip()
                .await
                .context("pixhawk stream closed")?;

            match message {
                PixhawkEvent::Gps { coords } => self.state.lock().unwrap().position = coords,
                PixhawkEvent::Orientation { attitude } => {
                    self.state.lock().unwrap().plane_attitude = attitude
                }
                _ => {}
            }

            if interrupt_recv.try_recv().is_ok() {
                break;
            }
        }

        Ok(())
    }
}

impl TelemetryPublisher {
    fn new(
        state: TelemetryState,
        sender: watch::Sender<Option<TelemetryInfo>>,
        channels: Arc<Channels>,
    ) -> Self {
        Self {
            state,
            sender,
            channels,
        }
    }

    async fn run(&self) -> anyhow::Result<()> {
        let telemetry_sender = self.channels.telemetry.clone();
        let mut interrupt_recv = self.channels.interrupt.subscribe();

        let mut interval = interval(Duration::from_millis(5));

        loop {
            if let Ok(telemetry) = self.state.lock() {
                if let Err(_) = self.sender.broadcast(Some(telemetry.clone())) {
                    break;
                }
            }

            if interrupt_recv.try_recv().is_ok() {
                break;
            }

            interval.tick().await;
        }

        Ok(())
    }
}

impl TelemetryStream {
    pub fn new(channels: Arc<Channels>, sender: watch::Sender<Option<TelemetryInfo>>) -> Self {
        let telemetry_state = Arc::new(Mutex::new(TelemetryInfo::default()));

        let collector = TelemetryCollector::new(telemetry_state.clone(), channels.clone());
        let publisher = TelemetryPublisher::new(telemetry_state.clone(), sender, channels.clone());

        Self {
            collector,
            publisher,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let collector = self.collector;
        let publisher = self.publisher;

        let collector_task = spawn(async move { collector.run().await });
        let publisher_task = spawn(async move { publisher.run().await });

        let (collector_result, publisher_result) =
            futures::future::join(collector_task, publisher_task).await;

        collector_result??;
        publisher_result??;

        Ok(())
    }
}
