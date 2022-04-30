use anyhow::Context;
use num_traits::{FromPrimitive, ToPrimitive};
use tokio::sync::broadcast;
use tokio::time::sleep;

use crate::util::retry_async;

use super::util::*;
use super::*;

macro_rules! get_camera_property {
    ($interface: expr, $prop: ident, $ty: ident) => {
        match $interface.get_value(CameraPropertyCode::$prop).await {
            Some(ptp::PtpData::$ty(v)) => Ok(Some(v)),
            Some(data) => Err(anyhow::anyhow!(
                "could not get {}, invalid value {:?}",
                stringify!($prop),
                data
            )),
            None => Ok(None),
        }
    };
}

//processes the status request
pub(super) async fn cmd_status(
    interface: CameraInterfaceRequestBuffer,
) -> anyhow::Result<CameraCommandResponse> {
    let (operating, exposure, focus, save_media, shutter_speed, iso, aperture, compression) =
        interface
            .enter(|i| async move {
                i.update().await?;

                let operating = get_camera_property!(i, OperatingMode, UINT8)?
                    .and_then(OperatingMode::from_u8)
                    .context("invalid operating mode")?;
                let compression = get_camera_property!(i, Compression, UINT8)?
                    .and_then(CompressionMode::from_u8)
                    .context("invalid compression mode")?;
                let exposure = get_camera_property!(i, ExposureMode, UINT16)?
                    .and_then(ExposureMode::from_u16)
                    .context("invalid exposure mode")?;
                let focus = get_camera_property!(i, FocusMode, UINT16)?
                    .and_then(FocusMode::from_u16)
                    .context("invalid focus mode")?;
                let save_media = get_camera_property!(i, SaveMedia, UINT16)?
                    .and_then(SaveMedia::from_u16)
                    .context("invalid save media")?;

                let shutter_speed =
                    get_camera_property!(i, ShutterSpeed, UINT32)?.and_then(ShutterSpeed::from_u32);
                let iso = get_camera_property!(i, ISO, UINT32)?.and_then(Iso::from_u32);
                let aperture =
                    get_camera_property!(i, FNumber, UINT16)?.and_then(Aperture::from_u16);
                Ok::<_, anyhow::Error>((
                    operating,
                    exposure,
                    focus,
                    save_media,
                    shutter_speed,
                    iso,
                    aperture,
                    compression,
                ))
            })
            .await
            .context("could not get status of camera")?;

    println!("operating mode: {operating:?}");
    println!("compression mode: {compression:?}");
    println!("exposure mode: {exposure:?}");
    println!("focus mode: {focus}");
    println!("save media: {save_media:?}");

    if let Some(aperture) = aperture {
        println!("aperture width: {aperture}");
    } else {
        println!("aperture width: <unknown>");
    }

    if let Some(iso) = iso {
        println!("iso: {iso}");
    } else {
        println!("iso: <unknown>");
    }

    if let Some(shutter_speed) = shutter_speed {
        println!("shutter speed: {shutter_speed}");
    } else {
        println!("shutter speed: <unknown>");
    }
    Ok(CameraCommandResponse::Status {
        operating_mode: operating,
        compression_mode: compression,
        exposure_mode: exposure,
        focus_mode: focus,
        save_mode: save_media,
        aperture: aperture,
        iso: iso,
        shutter_speed: shutter_speed,
    })
}

pub(super) async fn cmd_get(
    interface: CameraInterfaceRequestBuffer,
    req: CameraCommandGetRequest,
) -> anyhow::Result<CameraCommandResponse> {
    let prop = match req {
        CameraCommandGetRequest::ExposureMode => CameraPropertyCode::ExposureMode,
        CameraCommandGetRequest::OperatingMode => CameraPropertyCode::OperatingMode,
        CameraCommandGetRequest::SaveMode => CameraPropertyCode::SaveMedia,
        CameraCommandGetRequest::FocusMode => CameraPropertyCode::FocusMode,
        CameraCommandGetRequest::ZoomLevel => CameraPropertyCode::ZoomAbsolutePosition,
        CameraCommandGetRequest::CcInterval => CameraPropertyCode::IntervalTime,
        CameraCommandGetRequest::Other(_) => todo!(),
    };

    let prop_info = interface
        .enter(|i| async move { i.get_info(prop).await })
        .await
        .context("this property's value has not been retrieved")?;

    Ok(match req {
        CameraCommandGetRequest::ExposureMode => match prop_info.current {
            ptp::PtpData::UINT16(mode) => CameraCommandResponse::ExposureMode(
                FromPrimitive::from_u16(mode)
                    .context("invalid camera exposure mode (wrong value)")?,
            ),
            _ => bail!("invalid camera exposure mode (wrong data type)"),
        },
        CameraCommandGetRequest::OperatingMode => match prop_info.current {
            ptp::PtpData::UINT8(mode) => CameraCommandResponse::OperatingMode(
                FromPrimitive::from_u8(mode)
                    .context("invalid camera operating mode (wrong value)")?,
            ),
            _ => bail!("invalid camera operating mode (wrong data type)"),
        },
        CameraCommandGetRequest::SaveMode => match prop_info.current {
            ptp::PtpData::UINT16(mode) => CameraCommandResponse::SaveMode(
                FromPrimitive::from_u16(mode).context("invalid camera save mode (wrong value)")?,
            ),
            _ => bail!("invalid camera save mode (wrong data type)"),
        },
        CameraCommandGetRequest::FocusMode => match prop_info.current {
            ptp::PtpData::UINT16(mode) => CameraCommandResponse::FocusMode(
                FromPrimitive::from_u16(mode).context("invalid camera focus mode (wrong value)")?,
            ),
            _ => bail!("invalid camera focus mode (wrong data type)"),
        },
        CameraCommandGetRequest::ZoomLevel => match prop_info.current {
            ptp::PtpData::UINT16(level) => CameraCommandResponse::ZoomLevel(level as u8),
            _ => bail!("invalid camera zoom level (wrong data type)"),
        },
        CameraCommandGetRequest::CcInterval => match prop_info.current {
            ptp::PtpData::UINT16(interval) => {
                CameraCommandResponse::CcInterval(interval as f32 / 10.0)
            }
            _ => bail!("invalid camera zoom level (wrong data type)"),
        },
        CameraCommandGetRequest::Other(_) => todo!(),
    })
}

pub(super) async fn cmd_set(
    interface: CameraInterfaceRequestBuffer,
    req: CameraCommandSetRequest,
) -> anyhow::Result<CameraCommandResponse> {
    let (prop, data) = match req {
        CameraCommandSetRequest::ExposureMode { mode } => (
            CameraPropertyCode::ExposureMode,
            ptp::PtpData::UINT16(mode as u16),
        ),
        CameraCommandSetRequest::OperatingMode { mode } => (
            CameraPropertyCode::OperatingMode,
            ptp::PtpData::UINT8(mode as u8),
        ),
        CameraCommandSetRequest::SaveMode { mode } => (
            CameraPropertyCode::SaveMedia,
            ptp::PtpData::UINT16(mode as u16),
        ),
        CameraCommandSetRequest::FocusMode { mode } => (
            CameraPropertyCode::FocusMode,
            ptp::PtpData::UINT16(mode as u16),
        ),
        CameraCommandSetRequest::ZoomLevel { level } => (
            CameraPropertyCode::ZoomAbsolutePosition,
            ptp::PtpData::UINT16(level as u16),
        ),
        CameraCommandSetRequest::CcInterval { interval } => {
            let mut interval = (interval * 10.) as u16;

            if interval < 10 {
                bail!("minimum interval is 1 second");
            }

            if interval > 300 {
                bail!("maximum interval is 30 seconds");
            }

            if interval % 5 != 0 {
                warn!("valid intervals are in increments of 0.5 seconds; rounding down");
                interval -= interval % 5;
            }

            (
                CameraPropertyCode::IntervalTime,
                ptp::PtpData::UINT16(interval),
            )
        }
        CameraCommandSetRequest::ShutterSpeed { speed } => (
            CameraPropertyCode::ShutterSpeed,
            ptp::PtpData::UINT32(ToPrimitive::to_u32(&speed).unwrap()),
        ),
        CameraCommandSetRequest::Aperture { aperture } => (
            CameraPropertyCode::FNumber,
            ptp::PtpData::UINT16(ToPrimitive::to_u16(&aperture).unwrap()),
        ),
        // CameraCommandSetRequest::Other(s) => warn!("cannot set {"),
    };

    debug!("setting {:?} to {:x}", prop, data);

    ensure(&interface, prop, data).await?;

    Ok(CameraCommandResponse::Unit)
}

pub(super) async fn cmd_capture(
    interface: CameraInterfaceRequestBuffer,
    ptp_rx: &mut broadcast::Receiver<ptp::PtpEvent>,
) -> anyhow::Result<CameraCommandResponse> {
    ensure_mode(&interface, OperatingMode::StillRec).await?;

    interface
        .enter(|i| async move {
            info!("capturing image");

            debug!("sending half shutter press");

            // press shutter button halfway to fix the focus
            i.control(CameraControlCode::S1Button, ptp::PtpData::UINT16(0x0002))
                .await?;

            debug!("sending full shutter press");

            // shoot!
            i.control(CameraControlCode::S2Button, ptp::PtpData::UINT16(0x0002))
                .await?;

            debug!("sending full shutter release");

            // release
            i.control(CameraControlCode::S2Button, ptp::PtpData::UINT16(0x0001))
                .await?;

            debug!("sending half shutter release");

            // hell yeah
            i.control(CameraControlCode::S1Button, ptp::PtpData::UINT16(0x0001))
                .await?;

            Ok::<_, anyhow::Error>(())
        })
        .await?;

    info!("waiting for image confirmation");

    {
        let watch_fut = watch(&interface, CameraPropertyCode::ShootingFileInfo);
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
    interface: CameraInterfaceRequestBuffer,
    req: CameraCommandContinuousCaptureRequest,
) -> anyhow::Result<CameraCommandResponse> {
    match req {
        CameraCommandContinuousCaptureRequest::Start => {
            interface
                .enter(|i| async move {
                    i.control(
                        CameraControlCode::IntervalStillRecording,
                        ptp::PtpData::UINT16(0x0002),
                    )
                    .await
                    .context("failed to start interval recording")
                })
                .await?;
        }
        CameraCommandContinuousCaptureRequest::Stop => {
            interface
                .enter(|i| async move {
                    i.control(
                        CameraControlCode::IntervalStillRecording,
                        ptp::PtpData::UINT16(0x0001),
                    )
                    .await
                    .context("failed to start interval recording")
                })
                .await?;
        }
    }

    Ok(CameraCommandResponse::Unit)
}

pub(super) async fn cmd_storage(
    interface: CameraInterfaceRequestBuffer,
    req: CameraCommandStorageRequest,
) -> anyhow::Result<CameraCommandResponse> {
    match req {
        CameraCommandStorageRequest::List => {
            ensure_mode(&interface, OperatingMode::ContentsTransfer).await?;

            debug!("getting storage ids");

            sleep(Duration::from_secs(1)).await;

            debug!("checking for storage ID 0x00010000");

            interface
                .enter(|i| async move {
                    let storage_ids = i.storage_ids().await.context("could not get storage ids")?;

                    if storage_ids.contains(&ptp::StorageId::from(0x00010000)) {
                        bail!("no logical storage available");
                    }

                    debug!("got storage ids: {:?}", storage_ids);

                    let infos: Vec<Result<(_, _), _>> =
                        futures::future::join_all(storage_ids.iter().map(|&id| {
                            let i = &i;
                            async move { i.storage_info(id).await.map(|info| (id, info)) }
                        }))
                        .await;

                    infos
                        .into_iter()
                        .collect::<Result<HashMap<_, _>, _>>()
                        .map(|storages| CameraCommandResponse::StorageInfo { storages })
                })
                .await
        }
    }
}

pub(super) async fn cmd_file(
    interface: CameraInterfaceRequestBuffer,
    req: CameraCommandFileRequest,
    client_tx: broadcast::Sender<CameraClientEvent>,
) -> anyhow::Result<CameraCommandResponse> {
    match req {
        CameraCommandFileRequest::List { parent } => {
            ensure_mode(&interface, OperatingMode::ContentsTransfer).await?;

            debug!("getting object handles");

            interface
                .enter(|i| async move {
                    // wait for storage ID 0x00010001 to exist

                    retry_async(10, Some(Duration::from_secs(1)), || async {
                        debug!("checking for storage ID 0x00010001");

                        let storage_ids =
                            i.storage_ids().await.context("could not get storage ids")?;

                        if !storage_ids.contains(&ptp::StorageId::from(0x00010001)) {
                            bail!("no storage available");
                        } else {
                            Ok(())
                        }
                    })
                    .await?;

                    let object_handles = i
                        .object_handles(
                            ptp::StorageId::from(0x00010001),
                            parent
                                .clone()
                                .map(|v| ptp::ObjectHandle::from(v))
                                .unwrap_or(ptp::ObjectHandle::root()),
                        )
                        .await
                        .context("could not get object handles")?;

                    debug!("got object handles: {:?}", object_handles);

                    futures::future::join_all(object_handles.iter().map(|&id| {
                        let iface = &i;
                        async move { iface.object_info(id).await.map(|info| (id, info)) }
                    }))
                    .await
                    .into_iter()
                    .collect::<Result<HashMap<_, _>, _>>()
                    .map(|objects| CameraCommandResponse::ObjectInfo { objects })
                })
                .await
        }

        CameraCommandFileRequest::Get { handle: _ } => {
            ensure_mode(&interface, OperatingMode::ContentsTransfer).await?;

            let (info, data) = interface
                .enter(|i| async move {
                    let info = i
                        .object_info(ptp::ObjectHandle::from(0xFFFFC001))
                        .await
                        .context("failed to get object info for download")?;

                    let data = i
                        .object_data(ptp::ObjectHandle::from(0xFFFFC001))
                        .await
                        .context("failed to get object data for download")?;

                    Ok::<_, anyhow::Error>((info, data))
                })
                .await
                .context("downloading image data failed")?;

            let _ = client_tx.send(CameraClientEvent::Download {
                image_name: info.filename.clone(),
                image_data: Arc::new(data),
                cc_timestamp: None,
            });

            Ok(CameraCommandResponse::Download {
                name: info.filename,
            })
        }
    }
}
