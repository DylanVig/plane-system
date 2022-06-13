//! This module contains code for reading measurements from the current-sensing board.

use anyhow::Context;
use rppal::{gpio::*, i2c::*};

use crate::cli::config::CurrentSensingConfig;

pub async fn run_current_sensing(config: CurrentSensingConfig) -> anyhow::Result<()> {
    let gpio = Gpio::new().context("failed to access gpio")?;

    let mut i2c = config
        .i2c
        .map(|i2c_instance| {
            let mut i2c = I2c::with_bus(i2c_instance).context("failed to access i2c")?;
            i2c.set_slave_address(0b111_000)?;

            Ok::<_, anyhow::Error>(i2c)
        })
        .transpose()?;

    let mut pin_int = gpio
        .get(config.gpio_int)
        .context("failed to access interrupt gpio pin")?
        .into_input();

    let mut pin_ack = gpio
        .get(config.gpio_ack)
        .context("failed to access interrupt gpio pin")?
        .into_output_high();

    let (tx, rx) = flume::bounded(4);

    pin_int
        .set_async_interrupt(Trigger::Both, move |level| tx.send(level).unwrap())
        .context("failed to set irq handler")?;

    loop {
        let _ = rx.recv_async().await?;

        debug!("got csb interrupt");

        let timestamp = chrono::Local::now();

        let mut latitude = [0u8; 4];
        let mut longitude = [0u8; 4];

        if let Some(i2c) = &mut i2c {
            tokio::task::block_in_place(|| {
                i2c.read(&mut latitude[..])?;
                i2c.read(&mut longitude[..])?;
                Ok::<_, anyhow::Error>(())
            })?;
        }

        let latitude = u32::from_le_bytes(latitude);
        let longitude = u32::from_le_bytes(longitude);


        let coord = geo::Point::new(latitude as f32 / 1e4, longitude as f32 / 1e4);

        // TODO: do something with the coordinate

        pin_ack.set_low();

        // wait for interrupt pin to go high
        while let Level::Low = rx.recv_async().await? {}

        pin_ack.set_high();
    }
}
