//! This module contains code for reading measurements from the current-sensing board.

use std::sync::Arc;

use anyhow::Context;
use chrono::prelude::*;
use rppal::{gpio::*, i2c::*};
use serde::Serialize;
use tokio::sync::watch;

use crate::{cli::config::CurrentSensingConfig, Channels};

#[derive(Debug, Clone, Serialize)]
pub struct CurrentSensingTelemetry {
    #[serde(serialize_with = "crate::util::serialize_time")]
    pub timestamp: DateTime<Local>,
}

pub async fn run(
    channels: Arc<Channels>,
    csb_telemetry_tx: watch::Sender<Option<CurrentSensingTelemetry>>,
    config: CurrentSensingConfig,
) -> anyhow::Result<()> {
    let mut interrupt_recv = channels.interrupt.subscribe();

    info!("initializing csb routine");

    let gpio = Gpio::new().context("failed to access gpio")?;

    let mut i2c = config
        .i2c
        .map(|i2c_instance| {
            info!("intializing csb i2c");

            let mut i2c = I2c::with_bus(i2c_instance).context("failed to access i2c")?;

            debug!(
                "opened i2c bus {} at {} hz",
                i2c.bus(),
                i2c.clock_speed()
                    .context("failed to query i2c clock speed")?
            );

            i2c.set_slave_address(8)?;

            Ok::<_, anyhow::Error>(i2c)
        })
        .transpose()?;

    let mut pin_int = gpio
        .get(config.gpio_int)
        .context("failed to access interrupt gpio pin")?
        .into_input_pullup();

    let mut pin_ack = gpio
        .get(config.gpio_ack)
        .context("failed to access interrupt gpio pin")?
        .into_output_high();

    let (tx, rx) = flume::bounded(4);

    pin_int
        .set_async_interrupt(Trigger::Both, move |level| tx.send(level).unwrap())
        .context("failed to set irq handler")?;

    let loop_fut = async {
        loop {
            debug!("waiting for csb interrupt");

            while let Level::High = rx.recv_async().await? {
                debug!("int high");
            }

            debug!("got csb interrupt");

            let timestamp = chrono::Local::now();

            // let mut latitude = [0u8; 4];
            // let mut longitude = [0u8; 4];

            // if let Some(i2c) = &mut i2c {
            //     tokio::task::block_in_place(|| {
            //         i2c.read(&mut latitude[..])?;
            //         i2c.read(&mut longitude[..])?;
            //         Ok::<_, anyhow::Error>(())
            //     })?;
            // }

            // let latitude = u32::from_le_bytes(latitude);
            // let longitude = u32::from_le_bytes(longitude);

            // let coord = geo::Point::new(latitude as f32 / 1e4, longitude as f32 / 1e4);

            // TODO: do something with the coordinate

            debug!("setting ack low");

            pin_ack.set_low();

            debug!("waiting for int high");

            // wait for interrupt pin to go high
            while let Level::Low = rx.recv_async().await? {
                debug!("int low");
            }

            debug!("setting ack high");

            pin_ack.set_high();

            if let Err(err) = csb_telemetry_tx.send(Some(CurrentSensingTelemetry { timestamp })) {
                error!("failed to send csb event to broadcast channel: {:?}", err);
            }
        }

        #[allow(unreachable_code)]
        Ok::<_, anyhow::Error>(())
    };

    let interrupt_fut = interrupt_recv.recv();

    futures::pin_mut!(loop_fut);
    futures::pin_mut!(interrupt_fut);

    futures::future::select(loop_fut, interrupt_fut).await;

    Ok(())
}
