use crate::{pixhawk::state::PixhawkMessage, state::Telemetry, util::ReceiverExt, Channels};

use std::sync::{Arc, Mutex};

use anyhow::Context;
use std::time::Duration;
use tokio::spawn;
use tokio::time::interval;

// Noteworthy that this isn't a RwLock because we have at most one reader at any given moment
type TelemetryState = Arc<Mutex<Telemetry>>;

struct TelemetryCollector {
    telemetry_state: TelemetryState,
    channels: Arc<Channels>,
}

struct TelemetryPublisher {
    telemetry_state: TelemetryState,
    channels: Arc<Channels>,
}

pub struct TelemetryStream {
    telemetry_state: TelemetryState,
    channels: Arc<Channels>,
}

impl TelemetryCollector {
    fn new(telemetry_state: TelemetryState, channels: Arc<Channels>) -> Self {
        Self {
            telemetry_state,
            channels,
        }
    }

    async fn run(&self) -> anyhow::Result<()> {
        let mut pixhawk_recv = self.channels.pixhawk.subscribe();
        let mut interrupt_recv = self.channels.interrupt.subscribe();
        loop {
            let message = pixhawk_recv
                .recv_skip()
                .await
                .context("pixhawk stream closed")?;

            match message {
                PixhawkMessage::Gps { coords } => {
                    self.telemetry_state.lock().unwrap().position = coords
                }
                PixhawkMessage::Orientation { attitude } => {
                    self.telemetry_state.lock().unwrap().plane_attitude = attitude
                }
                _ => {}
            }
            if let Ok(_) = interrupt_recv.try_recv() {
                break;
            }
        }

        Ok(())
    }
}

impl TelemetryPublisher {
    fn new(telemetry_state: TelemetryState, channels: Arc<Channels>) -> Self {
        Self {
            telemetry_state,
            channels,
        }
    }

    async fn run(&self) -> anyhow::Result<()> {
        let telemetry_sender = self.channels.telemetry.clone();
        let mut interrupt_recv = self.channels.interrupt.subscribe();
        let mut interval = interval(Duration::from_millis(5));
        loop {
            if let Ok(telemetry) = self.telemetry_state.lock() {
                if let Err(_) = telemetry_sender.send(telemetry.clone()) {
                    break;
                }
            }
            if let Ok(_) = interrupt_recv.try_recv() {
                break;
            }
            interval.tick().await;
        }
        Ok(())
    }
}

impl TelemetryStream {
    pub fn new(channels: Arc<Channels>) -> Self {
        let telemetry_state = Arc::new(Mutex::new(Telemetry::default()));

        Self {
            telemetry_state,
            channels: channels,
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let collector =
            TelemetryCollector::new(self.telemetry_state.clone(), self.channels.clone());
        let publisher =
            TelemetryPublisher::new(self.telemetry_state.clone(), self.channels.clone());

        let collector_task = spawn(async move { collector.run().await });

        let publisher_task = spawn(async move { publisher.run().await });

        let (collector_result, publisher_result) =
            futures::future::join(collector_task, publisher_task).await;

        collector_result??;
        publisher_result??;
        Ok(())
    }
}
