use crate::{
    Channels,
    state::Telemetry,
    pixhawk::state::PixhawkMessage,
};

use std::sync::{Arc, Mutex};

use std::time::Duration;
use tokio::time::{timeout, delay_for};
use tokio::spawn;

pub struct TelemetryState {
    telemetry: Arc<Mutex<Telemetry>>,
    channels: Arc<Channels>,
}

impl TelemetryState {
    pub fn new(channels: Arc<Channels>) -> Self {
        Self {
            telemetry: Arc::new(Mutex::new(Telemetry::default())),
            channels: channels.clone(),/
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut pixhawk_recv = self.channels.pixhawk.subscribe();
        let mut interrupt_recv = self.channels.interrupt.subscribe();
        let telemetry = self.telemetry.clone();

        let telemetry_publisher = spawn(async move { self.publisher().await });
        
        loop {
            if let Ok(message) = timeout(Duration::from_millis(5), pixhawk_recv.recv()).await {
                match message? {
                    PixhawkMessage::Gps { coords } => telemetry.lock().unwrap().set_position(coords),
                    PixhawkMessage::Orientation { attitude } => telemetry.lock().unwrap().set_plane_attitude(attitude),
                    _ => {}
                }
            }
            // let _ = self.channels.telemetry.send(self.telemetry.lock().unwrap().clone());

            if let Ok(_) = timeout(Duration::from_millis(5), interrupt_recv.recv()).await { break; }
        }
        Ok(())
    }

    pub async fn publisher(&self) -> anyhow::Result<()> {
        let mut interrupt_recv = self.channels.interrupt.subscribe();
        loop {
            let _ = self.channels.telemetry.send(self.telemetry.lock().unwrap().clone());
            if let Ok(_) = interrupt_recv.try_recv() { break; }
            delay_for(Duration::from_millis(10)).await;
        }
        Ok(())
    }
}