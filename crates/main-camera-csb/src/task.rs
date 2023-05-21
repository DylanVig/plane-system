use std::time::Duration;

use anyhow::Context;
use defer::defer;
use log::{debug, info, warn, trace};
use rppal::{gpio::*, i2c::*};

use async_trait::async_trait;
use ps_client::Task;
use tokio::{select, io::AsyncWriteExt};
use tokio_util::sync::CancellationToken;

use crate::{CsbConfig, CsbEvent};

pub struct EventTask {
    pin_int: InputPin,
    pin_ack: OutputPin,
    evt_tx: flume::Sender<CsbEvent>,
    irq_rx: flume::Receiver<Level>,

    #[allow(dead_code)]
    // TODO: use I2C to read GPS values and such
    i2c: Option<I2c>,
}

pub fn create_task(config: CsbConfig) -> anyhow::Result<(EventTask, flume::Receiver<CsbEvent>)> {
    let gpio = Gpio::new().context("failed to access gpio")?;

    let i2c = config
        .i2c
        .map(|i2c_config| {
            info!("intializing csb i2c");

            let mut i2c = I2c::with_bus(i2c_config.bus).context("failed to access i2c")?;

            debug!(
                "opened i2c bus {} at {} hz",
                i2c.bus(),
                i2c.clock_speed()
                    .context("failed to query i2c clock speed")?
            );

            i2c.set_slave_address(i2c_config.addr)?;

            Ok::<_, anyhow::Error>(i2c)
        })
        .transpose()?;

    debug!("listening for interrupts on pin {}", config.gpio_int);

    let mut pin_int = gpio
        .get(config.gpio_int)
        .context("failed to access interrupt gpio pin")?
        .into_input_pullup();

    debug!("sending acks on pin {}", config.gpio_ack);

    let pin_ack = gpio
        .get(config.gpio_ack)
        .context("failed to access interrupt gpio pin")?
        .into_output_high();

    let (irq_tx, irq_rx) = flume::bounded(4);
    let (evt_tx, evt_rx) = flume::bounded(256);

    pin_int
        .set_async_interrupt(Trigger::Both, move |level| irq_tx.send(level).unwrap())
        .context("failed to set irq handler")?;

    Ok((
        EventTask {
            pin_int,
            pin_ack,
            irq_rx,
            i2c,
            evt_tx,
        },
        evt_rx,
    ))
}

#[async_trait]
impl Task for EventTask {
    fn name(&self) -> &'static str {
        "main-camera/csb/event"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            mut pin_ack,
            mut pin_int,
            evt_tx,
            irq_rx,
            mut i2c,
            ..
        } = *self;

        // log i2c level for debugging
        tokio::task::spawn({
            let cancel = cancel.clone();
            async move {
                let mut interval = tokio::time::interval(Duration::from_millis(50));
                let mut file = tokio::fs::File::create("csb.csv").await.unwrap();
                let mut file = tokio::io::BufWriter::new(file);

                while !cancel.is_cancelled() {
                    let mut reading = [0u8; 2];
                    // let mut longitude = [0u8; 4];

                    if let Some(i2c) = &mut i2c {
                        tokio::task::block_in_place(|| i2c.read(&mut reading[..])).unwrap();

                        let now = chrono::Utc::now();
                        let reading = u16::from_le_bytes(reading);
    
                        trace!("i2c says {reading} {reading:x}");
                        file.write_all(format!("{now},{reading}\n").as_bytes()).await.unwrap();
                    }

                    interval.tick().await;
                }
            }
        });

        let loop_fut = async move {
            // need to assign to a variable so it is not dropped until the end
            // of closure; when we drop we clear the interrupt handler
            let _m = defer(|| {
                let _ = pin_int.clear_async_interrupt();
            });

            loop {
                debug!("waiting for csb interrupt");

                while let Level::High = irq_rx.recv_async().await? {
                    debug!("int high");
                }

                debug!("got csb interrupt");

                let timestamp = chrono::Local::now();

                // let mut reading = [0u8; 2];
                // // let mut longitude = [0u8; 4];

                // if let Some(i2c) = &mut i2c {
                //     tokio::task::block_in_place(|| {
                //         i2c.read(&mut reading[..])?;
                //         Ok::<_, anyhow::Error>(())
                //     })?;
                // }

                // let reading = u16::from_le_bytes(reading);

                // debug!("i2c says {reading} {reading:x}");

                // let latitude = u32::from_le_bytes(latitude);
                // let longitude = u32::from_le_bytes(longitude);

                // let coord = geo::Point::new(latitude as f32 / 1e4, longitude as f32 / 1e4);

                // TODO: do something with the coordinate

                debug!("setting ack low");

                pin_ack.set_low();

                debug!("waiting for int high");

                // wait for interrupt pin to go high
                while let Level::Low = irq_rx.recv_async().await? {
                    debug!("int low");
                }

                debug!("setting ack high");

                pin_ack.set_high();

                if let Err(err) = evt_tx.send_async(CsbEvent { timestamp }).await {
                    warn!("failed publish csb event: {err:?}");
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
