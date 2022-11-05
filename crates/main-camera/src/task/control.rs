use std::{sync::Arc, time::Duration};

use anyhow::{bail, Context};
use async_trait::async_trait;
use log::{debug, error, info, trace, warn};
use num_traits::ToPrimitive;
use ps_client::{ChannelCommandSink, ChannelCommandSource, Task};
use ptp::{PtpData, PtpEvent};
use tokio::{
    select,
    sync::{oneshot, RwLock},
    time::{interval, sleep, timeout, MissedTickBehavior},
};
use tokio_util::sync::CancellationToken;

use super::{util::*, InterfaceGuard};
use crate::{
    interface::{ControlCode, OperatingMode, PropertyCode},
    Aperture, CameraContinuousCaptureRequest, CameraRequest, CameraResponse, CameraSetRequest,
    CameraZoomRequest, CompressionMode, DriveMode, ErrorMode, ExposureMode, FocusIndication,
    FocusMode, Iso, SaveMedia, ShutterSpeed,
};

pub struct ControlTask {
    interface: Arc<RwLock<InterfaceGuard>>,
    evt_rx: flume::Receiver<PtpEvent>,
    cmd_rx: ChannelCommandSource<CameraRequest, CameraResponse>,
    cmd_tx: ChannelCommandSink<CameraRequest, CameraResponse>,
}

impl ControlTask {
    pub(super) fn new(
        interface: Arc<RwLock<InterfaceGuard>>,
        evt_rx: flume::Receiver<PtpEvent>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = flume::bounded(256);

        Self {
            interface,
            evt_rx,
            cmd_rx,
            cmd_tx,
        }
    }

    pub fn cmd(&self) -> ChannelCommandSink<CameraRequest, CameraResponse> {
        self.cmd_tx.clone()
    }
}

#[async_trait]
impl Task for ControlTask {
    fn name(&self) -> &'static str {
        "main-camera/control"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let loop_fut = async move {
            let evt_rx = self.evt_rx;

            loop {
                match self.cmd_rx.recv_async().await {
                    Ok((req, ret)) => {
                        let interface = &*self.interface;

                        let result = match req {
                            CameraRequest::Storage(_) => todo!(),
                            CameraRequest::File(_) => todo!(),
                            CameraRequest::Capture {
                                burst_duration,
                                burst_high_speed,
                            } => {
                                run_capture(
                                    interface,
                                    evt_rx.clone(),
                                    burst_duration,
                                    burst_high_speed,
                                )
                                .await
                            }
                            CameraRequest::CCHack { interval, count } => {
                                let interface = self.interface.clone();
                                let cmd_tx = self.cmd_tx.clone();

                                tokio::task::spawn(async move {
                                    let mut counter = 0;
                                    let mut interval =
                                        tokio::time::interval(Duration::from_secs(interval as u64));

                                    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

                                    loop {
                                        interval.tick().await;
                                        info!("triggering capture from cc hack");

                                        let (tx, rx) = oneshot::channel();

                                        let res = cmd_tx
                                            .send_async((
                                                CameraRequest::Capture {
                                                    burst_duration: None,
                                                    burst_high_speed: false,
                                                },
                                                tx,
                                            ))
                                            .await;

                                        if res.is_err() {
                                            warn!("sending command from cc hack failed, exiting cc hack");
                                            break;
                                        }

                                        let res = rx.await;

                                        if let Err(err) = res {
                                            error!("error in capture: {err:?}");
                                        }

                                        if let Some(count) = count {
                                            if counter >= count {
                                                break;
                                            }
                                        }
                                    }

                                    info!("cc hack capture series done");
                                });

                                Ok(CameraResponse::Unit)
                            }
                            CameraRequest::Reset => run_reset(interface).await,
                            CameraRequest::Initialize => run_initialize(interface).await,
                            CameraRequest::Status => run_status(interface).await,
                            CameraRequest::Get(_) => todo!(),
                            CameraRequest::Set(req) => run_set(interface, req).await,
                            CameraRequest::ContinuousCapture(req) => run_cc(req, interface).await,
                            CameraRequest::Record(_) => todo!(),
                            CameraRequest::Zoom(req) => run_zoom(interface, req).await,
                        };

                        let _ = ret.send(result);
                    }
                    Err(_) => break,
                }
            }

            Ok::<_, anyhow::Error>(())
        };

        select! {
          _ = cancel.cancelled() => {}
          res = loop_fut => { res? }
        }

        Ok(())
    }
}

async fn run_status(interface: &RwLock<InterfaceGuard>) -> anyhow::Result<CameraResponse> {
    let props = interface.write().await.query()?;

    let sfi: u16 = convert_camera_value(&props, PropertyCode::ShootingFileInfo)?;
    let op_mode: OperatingMode = convert_camera_value(&props, PropertyCode::OperatingMode)?;
    let cmp_mode: CompressionMode = convert_camera_value(&props, PropertyCode::Compression)?;
    let ex_mode: ExposureMode = convert_camera_value(&props, PropertyCode::ExposureMode)?;
    let foc_mode: FocusMode = convert_camera_value(&props, PropertyCode::FocusMode)?;
    let save_media: SaveMedia = convert_camera_value(&props, PropertyCode::SaveMedia)?;
    let err_mode: ErrorMode = convert_camera_value(&props, PropertyCode::Caution)?;

    let interval_time: Option<u16> = convert_camera_value(&props, PropertyCode::IntervalTime).ok();
    let interval_state: Option<u8> =
        convert_camera_value(&props, PropertyCode::IntervalStillRecordingState).ok();
    let interval_state = interval_state.map(|s| s != 0);

    let shutter_speed: Option<ShutterSpeed> =
        convert_camera_value(&props, PropertyCode::ShutterSpeed).ok();
    let iso: Option<Iso> = convert_camera_value(&props, PropertyCode::ISO).ok();
    let aperture: Option<Aperture> = convert_camera_value(&props, PropertyCode::FNumber).ok();

    let shutter_speed = shutter_speed.map_or_else(|| "Unknown".to_owned(), |s| s.to_string());
    let iso = iso.map_or_else(|| "Unknown".to_owned(), |s| s.to_string());
    let aperture = aperture.map_or_else(|| "Unknown".to_owned(), |s| s.to_string());

    info!(
        "
        shooting file info: {sfi:#06x}
        operating mode: {op_mode:?}
        compression mode: {cmp_mode:?}
        exposure mode: {ex_mode:?}
        focus mode: {foc_mode:?}
        error mode: {err_mode:?}
        shutter speed: {shutter_speed}
        interval time: {interval_time:?}
        interval state: {interval_state:?}
        iso: {iso}
        aperture: {aperture}
        save destination: {save_media:?}
    "
    );

    Ok(CameraResponse::Unit)
}

async fn run_reset(interface: &RwLock<InterfaceGuard>) -> anyhow::Result<CameraResponse> {
    info!("resetting camera");

    ensure_camera_value(
        interface,
        PropertyCode::OperatingMode,
        PtpData::UINT8(OperatingMode::Standby as u8),
    )
    .await
    .context("failed to set camera to still recording mode")?;

    let mut interface = interface.write().await;

    interface.execute(ControlCode::CameraSettingReset, PtpData::UINT16(0x0002))?;

    sleep(Duration::from_secs(1)).await;

    interface.execute(ControlCode::CameraSettingReset, PtpData::UINT16(0x0001))?;

    Ok(CameraResponse::Unit)
}

async fn run_initialize(interface: &RwLock<InterfaceGuard>) -> anyhow::Result<CameraResponse> {
    info!("initializing camera");

    ensure_camera_value(
        interface,
        PropertyCode::OperatingMode,
        PtpData::UINT8(OperatingMode::Standby as u8),
    )
    .await
    .context("failed to set camera to still recording mode")?;

    let mut interface = interface.write().await;

    interface.execute(ControlCode::SystemInit, PtpData::UINT16(0x0002))?;

    sleep(Duration::from_secs(1)).await;

    interface.execute(ControlCode::SystemInit, PtpData::UINT16(0x0001))?;

    Ok(CameraResponse::Unit)
}

async fn run_zoom(
    interface: &RwLock<InterfaceGuard>,
    req: CameraZoomRequest,
) -> anyhow::Result<CameraResponse> {
    let mut interface = interface.write().await;

    match req {
        CameraZoomRequest::Wide { duration } => {
            interface.execute(ControlCode::ZoomControlWide, PtpData::UINT16(0x0002))?;
            sleep(Duration::from_millis(duration)).await;
            interface.execute(ControlCode::ZoomControlWide, PtpData::UINT16(0x0001))?;
        }
        CameraZoomRequest::Tele { duration } => {
            interface.execute(ControlCode::ZoomControlTele, PtpData::UINT16(0x0002))?;
            sleep(Duration::from_millis(duration)).await;
            interface.execute(ControlCode::ZoomControlTele, PtpData::UINT16(0x0001))?;
        }
        CameraZoomRequest::Level { level } => {
            //set target zoom level
            interface.set(
                PropertyCode::ZoomAbsolutePosition,
                ptp::PtpData::UINT8(level),
            );
            //do button press down
            interface.execute(ControlCode::ZoomControlAbsolute, PtpData::UINT16(0x0002))?;
            sleep(Duration::from_secs(1)).await;
            //do button press up
            interface.execute(ControlCode::ZoomControlAbsolute, PtpData::UINT16(0x0001))?;
        }
    }

    Ok(CameraResponse::Unit)
}

// pub(super) async fn cmd_get(
//     interface: CameraInterfaceRequestBuffer,
//     req: CameraCommandGetRequest,
// ) -> anyhow::Result<CameraResponse> {
//     let prop = match req {
//         CameraCommandGetRequest::ExposureMode => PropertyCode::ExposureMode,
//         CameraCommandGetRequest::OperatingMode => PropertyCode::OperatingMode,
//         CameraCommandGetRequest::SaveMode => PropertyCode::SaveMedia,
//         CameraCommandGetRequest::FocusMode => PropertyCode::FocusMode,
//         CameraCommandGetRequest::ZoomLevel => PropertyCode::ZoomAbsolutePosition,
//         CameraCommandGetRequest::CcInterval => PropertyCode::IntervalTime,
//         CameraCommandGetRequest::Other(_) => todo!(),
//     };

//     let prop_info = interface
//         .enter(|i| async move { i.get_info(prop).await })
//         .await
//         .context("this property's value has not been retrieved")?;

//     Ok(match req {
//         CameraCommandGetRequest::ExposureMode => match prop_info.current {
//             ptp::PtpData::UINT16(mode) => CameraResponse::ExposureMode(
//                 FromPrimitive::from_u16(mode)
//                     .context("invalid camera exposure mode (wrong value)")?,
//             ),
//             _ => bail!("invalid camera exposure mode (wrong data type)"),
//         },
//         CameraCommandGetRequest::OperatingMode => match prop_info.current {
//             ptp::PtpData::UINT8(mode) => CameraResponse::OperatingMode(
//                 FromPrimitive::from_u8(mode)
//                     .context("invalid camera operating mode (wrong value)")?,
//             ),
//             _ => bail!("invalid camera operating mode (wrong data type)"),
//         },
//         CameraCommandGetRequest::SaveMode => match prop_info.current {
//             ptp::PtpData::UINT16(mode) => CameraResponse::SaveMode(
//                 FromPrimitive::from_u16(mode).context("invalid camera save mode (wrong value)")?,
//             ),
//             _ => bail!("invalid camera save mode (wrong data type)"),
//         },
//         CameraCommandGetRequest::FocusMode => match prop_info.current {
//             ptp::PtpData::UINT16(mode) => CameraResponse::FocusMode(
//                 FromPrimitive::from_u16(mode).context("invalid camera focus mode (wrong value)")?,
//             ),
//             _ => bail!("invalid camera focus mode (wrong data type)"),
//         },
//         CameraCommandGetRequest::ZoomLevel => match prop_info.current {
//             ptp::PtpData::UINT16(level) => CameraResponse::ZoomLevel(level as u8),
//             _ => bail!("invalid camera zoom level (wrong data type)"),
//         },
//         CameraCommandGetRequest::CcInterval => match prop_info.current {
//             ptp::PtpData::UINT16(interval) => CameraResponse::CcInterval(interval as f32 / 10.0),
//             _ => bail!("invalid camera zoom level (wrong data type)"),
//         },
//         CameraCommandGetRequest::Other(_) => todo!(),
//     })
// }

pub(super) async fn run_set(
    interface: &RwLock<InterfaceGuard>,
    req: CameraSetRequest,
) -> anyhow::Result<CameraResponse> {
    let (prop, data) = match req {
        CameraSetRequest::ExposureMode { mode } => (
            PropertyCode::ExposureMode,
            ptp::PtpData::UINT16(mode as u16),
        ),
        CameraSetRequest::OperatingMode { mode } => {
            (PropertyCode::OperatingMode, ptp::PtpData::UINT8(mode as u8))
        }
        CameraSetRequest::SaveMode { mode } => {
            (PropertyCode::SaveMedia, ptp::PtpData::UINT16(mode as u16))
        }
        CameraSetRequest::FocusMode { mode } => {
            (PropertyCode::FocusMode, ptp::PtpData::UINT16(mode as u16))
        }
        CameraSetRequest::CcInterval { interval } => {
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

            (PropertyCode::IntervalTime, ptp::PtpData::UINT16(interval))
        }
        CameraSetRequest::ShutterSpeed { speed } => (
            PropertyCode::ShutterSpeed,
            ptp::PtpData::UINT32(ToPrimitive::to_u32(&speed).unwrap()),
        ),
        CameraSetRequest::Aperture { aperture } => (
            PropertyCode::FNumber,
            ptp::PtpData::UINT16(ToPrimitive::to_u16(&aperture).unwrap()),
        ),
    };

    debug!("setting {:?} to {:x}", prop, data);

    ensure_camera_value(&interface, prop, data).await?;

    Ok(CameraResponse::Unit)
}

pub(super) async fn run_capture(
    interface: &RwLock<InterfaceGuard>,
    evt_rx: flume::Receiver<PtpEvent>,
    burst_duration: Option<u8>,
    burst_high_speed: bool,
) -> anyhow::Result<CameraResponse> {
    debug!("running camera capture");

    ensure_camera_value(
        interface,
        PropertyCode::OperatingMode,
        PtpData::UINT8(OperatingMode::StillRec as u8),
    )
    .await
    .context("failed to set camera to still recording mode")?;

    ensure_camera_value(
        interface,
        PropertyCode::DriveMode,
        PtpData::UINT16(if burst_duration.is_some() {
            if burst_high_speed {
                DriveMode::SpeedPriorityContinuousShot
            } else {
                DriveMode::ContinuousShot
            }
        } else {
            DriveMode::Normal
        } as u16),
    )
    .await
    .context("failed to set camera to correct drive mode")?;

    {
        let mut interface = interface.write().await;
        let props = interface
            .query()
            .context("failed to query current camera state")?;

        let focus_mode: FocusMode = convert_camera_value(&props, PropertyCode::FocusMode)?;

        info!("capturing image (focus mode = {focus_mode}, burst duration = {burst_duration:?})");

        // press shutter button halfway to fix the focus
        debug!("sending half shutter press");
        interface.execute(ControlCode::S1Button, PtpData::UINT16(0x0002))?;

        if focus_mode == FocusMode::AutoFocusStill {
            loop {
                let props = interface
                    .query()
                    .context("failed to query current camera state")?;

                let focus_ind: FocusIndication =
                    convert_camera_value(&props, PropertyCode::FocusIndication)?;

                match focus_ind {
                    FocusIndication::AFUnlock | FocusIndication::Focusing => {
                        debug!("focusing ({focus_ind})")
                    }
                    FocusIndication::AFLock | FocusIndication::FocusedContinuous => {
                        debug!("focused ({focus_ind})");
                        break;
                    }
                    FocusIndication::AFWarning => {
                        debug!("focus failed ({focus_ind})");
                        debug!("sending half shutter release");
                        interface.execute(ControlCode::S1Button, PtpData::UINT16(0x0001))?;
                        bail!("failed to acquire auto focus lock");
                    }
                }
            }
        }

        // shoot!
        debug!("sending full shutter press");
        interface.execute(ControlCode::S2Button, PtpData::UINT16(0x0002))?;

        if let Some(burst_duration) = burst_duration {
            sleep(Duration::from_secs(burst_duration as u64)).await;
        }

        // release
        debug!("sending full shutter release");
        interface.execute(ControlCode::S2Button, PtpData::UINT16(0x0001))?;

        // hell yeah
        debug!("sending half shutter release");
        interface.execute(ControlCode::S1Button, PtpData::UINT16(0x0001))?;
    }

    info!("waiting for image confirmation");

    timeout(Duration::from_millis(3000), async {
        loop {
            // TODO: maybe check ShootingFileInfo

            let evt = evt_rx.recv_async().await?;

            match evt.code {
                ptp::EventCode::Vendor(0xC204) | ptp::EventCode::Vendor(0xC203) => {
                    break;
                }
                _ => {}
            }
        }

        Ok::<_, anyhow::Error>(())
    })
    .await
    .context("timed out while waiting for image confirmation")?
    .context("error while waiting for image confirmation")?;

    Ok(CameraResponse::Unit)
}

pub(super) async fn run_cc(
    req: CameraContinuousCaptureRequest,
    interface: &RwLock<InterfaceGuard>,
) -> anyhow::Result<CameraResponse> {
    match req {
        CameraContinuousCaptureRequest::Start => {
            interface
                .write()
                .await
                .execute(ControlCode::IntervalStillRecording, PtpData::UINT16(0x0002))
                .context("failed to start interval recording")?;
        }

        CameraContinuousCaptureRequest::Stop => {
            interface
                .write()
                .await
                .execute(ControlCode::IntervalStillRecording, PtpData::UINT16(0x0001))
                .context("failed to start interval recording")?;
        }
    }

    Ok(CameraResponse::Unit)
}

// pub(super) async fn cmd_storage(
//     interface: CameraInterfaceRequestBuffer,
//     req: CameraCommandStorageRequest,
// ) -> anyhow::Result<CameraResponse> {
//     match req {
//         CameraCommandStorageRequest::List => {
//             ensure_mode(&interface, OperatingMode::ContentsTransfer).await?;

//             debug!("getting storage ids");

//             sleep(Duration::from_secs(1)).await;

//             debug!("checking for storage ID 0x00010000");

//             interface
//                 .enter(|i| async move {
//                     let storage_ids = i.storage_ids().await.context("could not get storage ids")?;

//                     if storage_ids.contains(&ptp::StorageId::from(0x00010000)) {
//                         bail!("no logical storage available");
//                     }

//                     debug!("got storage ids: {:?}", storage_ids);

//                     let infos: Vec<Result<(_, _), _>> =
//                         futures::future::join_all(storage_ids.iter().map(|&id| {
//                             let i = &i;
//                             async move { i.storage_info(id).await.map(|info| (id, info)) }
//                         }))
//                         .await;

//                     infos
//                         .into_iter()
//                         .collect::<Result<HashMap<_, _>, _>>()
//                         .map(|storages| CameraResponse::StorageInfo { storages })
//                 })
//                 .await
//         }
//     }
// }

// pub(super) async fn cmd_file(
//     interface: CameraInterfaceRequestBuffer,
//     req: CameraCommandFileRequest,
//     client_tx: broadcast::Sender<CameraClientEvent>,
// ) -> anyhow::Result<CameraResponse> {
//     match req {
//         CameraCommandFileRequest::List { parent } => {
//             ensure_mode(&interface, OperatingMode::ContentsTransfer).await?;

//             debug!("getting object handles");

//             interface
//                 .enter(|i| async move {
//                     // wait for storage ID 0x00010001 to exist

//                     retry_async(10, Some(Duration::from_secs(1)), || async {
//                         debug!("checking for storage ID 0x00010001");

//                         let storage_ids =
//                             i.storage_ids().await.context("could not get storage ids")?;

//                         if !storage_ids.contains(&ptp::StorageId::from(0x00010001)) {
//                             bail!("no storage available");
//                         } else {
//                             Ok(())
//                         }
//                     })
//                     .await?;

//                     let object_handles = i
//                         .object_handles(
//                             ptp::StorageId::from(0x00010001),
//                             parent
//                                 .clone()
//                                 .map(|v| ptp::ObjectHandle::from(v))
//                                 .unwrap_or(ptp::ObjectHandle::root()),
//                         )
//                         .await
//                         .context("could not get object handles")?;

//                     debug!("got object handles: {:?}", object_handles);

//                     futures::future::join_all(object_handles.iter().map(|&id| {
//                         let iface = &i;
//                         async move { iface.object_info(id).await.map(|info| (id, info)) }
//                     }))
//                     .await
//                     .into_iter()
//                     .collect::<Result<HashMap<_, _>, _>>()
//                     .map(|objects| CameraResponse::ObjectInfo { objects })
//                 })
//                 .await
//         }

//         CameraCommandFileRequest::Get { handle: _ } => {
//             ensure_mode(&interface, OperatingMode::ContentsTransfer).await?;

//             let (info, data) = interface
//                 .enter(|i| async move {
//                     let info = i
//                         .object_info(ptp::ObjectHandle::from(0xFFFFC001))
//                         .await
//                         .context("failed to get object info for download")?;

//                     let data = i
//                         .object_data(ptp::ObjectHandle::from(0xFFFFC001))
//                         .await
//                         .context("failed to get object data for download")?;

//                     Ok::<_, anyhow::Error>((info, data))
//                 })
//                 .await
//                 .context("downloading image data failed")?;

//             let _ = client_tx.send(CameraClientEvent::Download {
//                 image_name: info.filename.clone(),
//                 image_data: Arc::new(data),
//                 cc_timestamp: None,
//             });

//             Ok(CameraResponse::Download {
//                 name: info.filename,
//             })
//         }
//     }
// }
