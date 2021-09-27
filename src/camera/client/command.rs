use anyhow::Context;
use num_traits::FromPrimitive;
use std::{sync::Arc, time::Duration};
use tokio::sync::{broadcast, RwLock};

use super::util::*;
use super::*;

pub(super) async fn cmd_debug(
    client: Arc<RwLock<CameraClient>>,
    req: CameraCommandDebugRequest,
) -> anyhow::Result<CameraCommandResponse> {
    let client = &mut *client.write().await;

    if let Some(property) = req.property {
        let property_code: CameraPropertyCode =
            FromPrimitive::from_u32(property).context("not a valid camera property code")?;
        println!("dumping {:#X?}", property_code);

        let property = client.state.get(&property_code);
        println!("dumping {:#X?}", property);

        if let Some(property) = property {
            if let Some(&value) = req.value_num.first() {
                let property_value = match property.data_type {
                    0x0001 => ptp::PtpData::INT8(value as i8),
                    0x0002 => ptp::PtpData::UINT8(value as u8),
                    0x0003 => ptp::PtpData::INT16(value as i16),
                    0x0004 => ptp::PtpData::UINT16(value as u16),
                    0x0005 => ptp::PtpData::INT32(value as i32),
                    0x0006 => ptp::PtpData::UINT32(value as u32),
                    0x0007 => ptp::PtpData::INT64(value as i64),
                    0x0008 => ptp::PtpData::UINT64(value as u64),
                    _ => bail!("cannot set this property type, not implemented"),
                };

                println!("setting {:#X?} to {:#X?}", property_code, property_value);

                ensure(client, property_code, property_value).await?;
            }
        }
    } else {
        println!("{:#X?}", client.state);
    }

    Ok(CameraCommandResponse::Unit)
}

pub(super) async fn cmd_capture(
    client: Arc<RwLock<CameraClient>>,
    ptp_rx: &mut broadcast::Receiver<ptp::PtpEvent>,
) -> anyhow::Result<CameraCommandResponse> {
    let client = &*client;

    {
        let mut client = client.write().await;

        ensure_mode(&mut *client, CameraOperatingMode::StillRec).await?;

        info!("capturing image");

        debug!("sending half shutter press");
        // press shutter button halfway to fix the focus
        control(
            &mut client.interface,
            CameraControlCode::S1Button,
            ptp::PtpData::UINT16(0x0002),
        )?;

        debug!("sending full shutter press");

        // shoot!
        control(
            &mut client.interface,
            CameraControlCode::S2Button,
            ptp::PtpData::UINT16(0x0002),
        )?;

        debug!("sending full shutter release");

        // release
        control(
            &mut client.interface,
            CameraControlCode::S2Button,
            ptp::PtpData::UINT16(0x0001),
        )?;

        debug!("sending half shutter release");

        // hell yeah
        control(
            &mut client.interface,
            CameraControlCode::S1Button,
            ptp::PtpData::UINT16(0x0001),
        )?;

        info!("waiting for image confirmation");
    }

    {
        let watch_fut = watch(client, CameraPropertyCode::ShootingFileInfo);
        let wait_fut = wait(ptp_rx, ptp::EventCode::Vendor(0xC204));

        futures::pin_mut!(watch_fut);
        futures::pin_mut!(wait_fut);

        let confirm_fut = futures::future::select(watch_fut, wait_fut);

        let res = tokio::time::timeout(Duration::from_millis(3000), confirm_fut)
            .await
            .context("timed out while waiting for image confirmation")?;

        match res {
            futures::future::Either::Left((watch_res, _)) => {
                watch_res.context("error while waiting for change in shooting file counter")?;
            }
            futures::future::Either::Right((wait_res, _)) => {
                wait_res.context("error while waiting for capture complete event")?;
            }
        }
    }

    Ok(CameraCommandResponse::Unit)
}

pub(super) async fn cmd_continuous_capture(
    client: Arc<RwLock<CameraClient>>,
    req: CameraCommandContinuousCaptureRequest,
) -> anyhow::Result<CameraCommandResponse> {
    let mut client = client.write().await;

    match req {
        CameraCommandContinuousCaptureRequest::Start => {
            control(
                &mut client.interface,
                CameraControlCode::IntervalStillRecording,
                ptp::PtpData::UINT16(0x0002),
            )
            .context("failed to start interval recording")?;
        }
        CameraCommandContinuousCaptureRequest::Stop => {
            control(
                &mut client.interface,
                CameraControlCode::IntervalStillRecording,
                ptp::PtpData::UINT16(0x0001),
            )
            .context("failed to stop interval recording")?;
        }
        CameraCommandContinuousCaptureRequest::Interval { interval } => {
            let interval = (interval * 10.) as u16;

            if interval < 10 {
                bail!("minimum interval is 1 second");
            }

            if interval > 300 {
                bail!("maximum interval is 30 seconds");
            }

            if interval % 5 != 0 {
                bail!("valid intervals are in increments of 0.5 seconds");
            }

            ensure(
                &mut client,
                CameraPropertyCode::IntervalTime,
                ptp::PtpData::UINT16(interval),
            )
            .await
            .context("failed to set camera interval")?;
        }
    }

    Ok(CameraCommandResponse::Unit)
}
