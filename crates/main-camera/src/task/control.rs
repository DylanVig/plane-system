use std::sync::Arc;

use async_trait::async_trait;
use ps_client::{ChannelCommandSource, Task};
use tokio::{select, sync::RwLock};
use tokio_util::sync::CancellationToken;

use super::util::*;
use crate::{
    interface::{CameraInterface, OperatingMode, PropertyCode},
    Aperture, CameraRequest, CameraResponse, CompressionMode, ErrorMode, ExposureMode, FocusMode,
    Iso, SaveMedia, ShutterSpeed,
};

pub struct ControlTask {
    pub(super) interface: Arc<RwLock<CameraInterface>>,
    pub(super) cmd_rx: ChannelCommandSource<CameraRequest, CameraResponse>,
}

#[async_trait]
impl Task for ControlTask {
    fn name() -> &'static str {
        "main-camera/control"
    }

    async fn run(self, cancel: CancellationToken) -> anyhow::Result<()> {
        let loop_fut = async move {
            loop {
                match self.cmd_rx.recv_async().await {
                    Ok((req, ret)) => {
                        let interface = &*self.interface;
                        let result = match req {
                            CameraRequest::Storage(_) => todo!(),
                            CameraRequest::File(_) => todo!(),
                            CameraRequest::Capture => todo!(),
                            CameraRequest::Reconnect => todo!(),
                            CameraRequest::Status => run_status(interface),
                            CameraRequest::Get(_) => todo!(),
                            CameraRequest::Set(_) => todo!(),
                            CameraRequest::ContinuousCapture(_) => todo!(),
                            CameraRequest::Record(_) => todo!(),
                        }
                        .await;

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

async fn run_status(interface: &RwLock<CameraInterface>) -> anyhow::Result<CameraResponse> {
    let props = get_camera_values(interface).await?;

    let op_mode: OperatingMode = convert_camera_value(&props, PropertyCode::OperatingMode)?;
    let cmp_mode: CompressionMode = convert_camera_value(&props, PropertyCode::Compression)?;
    let ex_mode: ExposureMode = convert_camera_value(&props, PropertyCode::ExposureMode)?;
    let foc_mode: FocusMode = convert_camera_value(&props, PropertyCode::FocusMode)?;
    let save_media: SaveMedia = convert_camera_value(&props, PropertyCode::SaveMedia)?;
    let err_mode: ErrorMode = convert_camera_value(&props, PropertyCode::Caution)?;
    let shutter_speed: ShutterSpeed = convert_camera_value(&props, PropertyCode::ShutterSpeed)?;
    let iso: Iso = convert_camera_value(&props, PropertyCode::ISO)?;
    let aperture: Aperture = convert_camera_value(&props, PropertyCode::FNumber)?;

    todo!()
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

// pub(super) async fn cmd_set(
//     interface: CameraInterfaceRequestBuffer,
//     req: CameraCommandSetRequest,
// ) -> anyhow::Result<CameraResponse> {
//     let (prop, data) = match req {
//         CameraCommandSetRequest::ExposureMode { mode } => (
//             PropertyCode::ExposureMode,
//             ptp::PtpData::UINT16(mode as u16),
//         ),
//         CameraCommandSetRequest::OperatingMode { mode } => {
//             (PropertyCode::OperatingMode, ptp::PtpData::UINT8(mode as u8))
//         }
//         CameraCommandSetRequest::SaveMode { mode } => {
//             (PropertyCode::SaveMedia, ptp::PtpData::UINT16(mode as u16))
//         }
//         CameraCommandSetRequest::FocusMode { mode } => {
//             (PropertyCode::FocusMode, ptp::PtpData::UINT16(mode as u16))
//         }
//         CameraCommandSetRequest::ZoomLevel { level } => (
//             PropertyCode::ZoomAbsolutePosition,
//             ptp::PtpData::UINT16(level as u16),
//         ),
//         CameraCommandSetRequest::CcInterval { interval } => {
//             let mut interval = (interval * 10.) as u16;

//             if interval < 10 {
//                 bail!("minimum interval is 1 second");
//             }

//             if interval > 300 {
//                 bail!("maximum interval is 30 seconds");
//             }

//             if interval % 5 != 0 {
//                 warn!("valid intervals are in increments of 0.5 seconds; rounding down");
//                 interval -= interval % 5;
//             }

//             (PropertyCode::IntervalTime, ptp::PtpData::UINT16(interval))
//         }
//         CameraCommandSetRequest::ShutterSpeed { speed } => (
//             PropertyCode::ShutterSpeed,
//             ptp::PtpData::UINT32(ToPrimitive::to_u32(&speed).unwrap()),
//         ),
//         CameraCommandSetRequest::Aperture { aperture } => (
//             PropertyCode::FNumber,
//             ptp::PtpData::UINT16(ToPrimitive::to_u16(&aperture).unwrap()),
//         ),
//         // CameraCommandSetRequest::Other(s) => warn!("cannot set {"),
//     };

//     debug!("setting {:?} to {:x}", prop, data);

//     ensure(&interface, prop, data).await?;

//     Ok(CameraResponse::Unit)
// }

// pub(super) async fn cmd_capture(
//     interface: CameraInterfaceRequestBuffer,
//     ptp_rx: &mut broadcast::Receiver<ptp::PtpEvent>,
// ) -> anyhow::Result<CameraResponse> {
//     ensure_mode(&interface, OperatingMode::StillRec).await?;

//     interface
//         .enter(|i| async move {
//             info!("capturing image");

//             debug!("sending half shutter press");

//             // press shutter button halfway to fix the focus
//             i.control(CameraControlCode::S1Button, ptp::PtpData::UINT16(0x0002))
//                 .await?;

//             debug!("sending full shutter press");

//             // shoot!
//             i.control(CameraControlCode::S2Button, ptp::PtpData::UINT16(0x0002))
//                 .await?;

//             debug!("sending full shutter release");

//             // release
//             i.control(CameraControlCode::S2Button, ptp::PtpData::UINT16(0x0001))
//                 .await?;

//             debug!("sending half shutter release");

//             // hell yeah
//             i.control(CameraControlCode::S1Button, ptp::PtpData::UINT16(0x0001))
//                 .await?;

//             Ok::<_, anyhow::Error>(())
//         })
//         .await?;

//     info!("waiting for image confirmation");

//     {
//         let watch_fut = watch(&interface, PropertyCode::ShootingFileInfo);
//         let wait_fut = wait(ptp_rx, ptp::EventCode::Vendor(0xC204));

//         futures::pin_mut!(watch_fut);
//         futures::pin_mut!(wait_fut);

//         let confirm_fut = futures::future::select(watch_fut, wait_fut);

//         let res = tokio::time::timeout(Duration::from_millis(3000), confirm_fut)
//             .await
//             .context("timed out while waiting for image confirmation")?;

//         match res {
//             futures::future::Either::Left((watch_res, _)) => {
//                 watch_res.context("error while waiting for change in shooting file counter")?;
//             }
//             futures::future::Either::Right((wait_res, _)) => {
//                 wait_res.context("error while waiting for capture complete event")?;
//             }
//         }
//     }

//     Ok(CameraResponse::Unit)
// }

// pub(super) async fn cmd_continuous_capture(
//     interface: CameraInterfaceRequestBuffer,
//     req: CameraCommandContinuousCaptureRequest,
// ) -> anyhow::Result<CameraResponse> {
//     match req {
//         CameraCommandContinuousCaptureRequest::Start => {
//             interface
//                 .enter(|i| async move {
//                     i.control(
//                         CameraControlCode::IntervalStillRecording,
//                         ptp::PtpData::UINT16(0x0002),
//                     )
//                     .await
//                     .context("failed to start interval recording")
//                 })
//                 .await?;
//         }
//         CameraCommandContinuousCaptureRequest::Stop => {
//             interface
//                 .enter(|i| async move {
//                     i.control(
//                         CameraControlCode::IntervalStillRecording,
//                         ptp::PtpData::UINT16(0x0001),
//                     )
//                     .await
//                     .context("failed to start interval recording")
//                 })
//                 .await?;
//         }
//     }

//     Ok(CameraResponse::Unit)
// }

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
