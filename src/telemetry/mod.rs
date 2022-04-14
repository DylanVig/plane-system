use crate::{pixhawk::state::PixhawkEvent, state::Telemetry, util::ReceiverExt, Channels};

use std::sync::{Arc, Mutex};

use anyhow::Context;
use std::time::Duration;
use tokio::time::interval;
use tokio::{spawn, sync::watch};

// Noteworthy that this isn't a RwLock because we have at most one reader at any given moment
type TelemetryState = Arc<Mutex<Telemetry>>;

struct TelemetryCollector {
    state: TelemetryState,
    channels: Arc<Channels>,
}

struct TelemetryPublisher {
    state: TelemetryState,
    sender: watch::Sender<Option<Telemetry>>,
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
        let mut interrupt_recv = self.channels.interrupt.subscribe();
        let interrupt_fut = interrupt_recv.recv();

        // pixhawk_recv can block indefinitely if the pixhawk is disabled; there
        // is no cleanup for telemetry stream so we can just do a select
        let loop_fut = async {
            let mut pixhawk_recv = self.channels.pixhawk_event.subscribe();

            loop {
                let message = pixhawk_recv
                    .recv_skip()
                    .await
                    .context("pixhawk stream closed")?;

                match message {
                    PixhawkEvent::Gps { position, velocity } => {
                        let mut state = self.state.lock().unwrap();
                        state.position = position;
                        state.velocity = velocity;
                        state.time = chrono::Local::now();
                    }
                    PixhawkEvent::Orientation { attitude } => {
                        let mut state = self.state.lock().unwrap();
                        state.plane_attitude = attitude;
                        state.time = chrono::Local::now();
                    }
                    _ => {}
                }
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
}

impl TelemetryPublisher {
    fn new(
        state: TelemetryState,
        sender: watch::Sender<Option<Telemetry>>,
        channels: Arc<Channels>,
    ) -> Self {
        Self {
            state,
            sender,
            channels,
        }
    }

    async fn run(&self) -> anyhow::Result<()> {
        let mut interrupt_recv = self.channels.interrupt.subscribe();

        let mut interval = interval(Duration::from_millis(500));

        loop {
            if let Ok(telemetry) = self.state.lock() {
                if let Err(_) = self.sender.send(Some(telemetry.clone())) {
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
    pub fn new(channels: Arc<Channels>, sender: watch::Sender<Option<Telemetry>>) -> Self {
        let telemetry_state = Arc::new(Mutex::new(Telemetry::default()));

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
