use std::{collections::HashMap, str::FromStr};

use anyhow::bail;
use clap::Subcommand;
use serde::Serialize;

use super::{interface::OperatingMode, state::*};

#[derive(Subcommand, Debug, Clone)]
pub enum CameraRequest {
    /// view information about the storage media inside of the camera
    #[clap(subcommand)]
    Storage(CameraCommandStorageRequest),

    /// view information about the files stored on the camera; download files
    #[clap(subcommand)]
    File(CameraCommandFileRequest),

    /// capture an image
    Capture,

    /// disconnect and reconnect to the camera
    Reconnect,

    Status,

    /// get a property of the camera's state
    #[clap(subcommand)]
    Get(CameraCommandGetRequest),

    /// set a property of the camera's state
    #[clap(subcommand)]
    Set(CameraCommandSetRequest),

    /// control continuous capture
    #[clap(name = "cc")]
    #[clap(subcommand)]
    ContinuousCapture(CameraCommandContinuousCaptureRequest),

    /// record videos
    #[clap(subcommand)]
    Record(CameraCommandRecordRequest),
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraCommandGetRequest {
    ExposureMode,
    OperatingMode,
    SaveMode,
    FocusMode,
    ZoomLevel,
    CcInterval,

    #[clap(external_subcommand)]
    Other(Vec<String>),
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraCommandSetRequest {
    ExposureMode { mode: ExposureMode },
    OperatingMode { mode: OperatingMode },
    SaveMode { mode: SaveMedia },
    FocusMode { mode: FocusMode },
    ZoomLevel { level: u16 },
    CcInterval { interval: f32 },
    ShutterSpeed { speed: ShutterSpeed },
    Aperture { aperture: Aperture },
    // #[clap(external_subcommand)]
    // Other(Vec<String>),
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraCommandStorageRequest {
    /// list the storage volumes available on the camera
    List,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraCommandFileRequest {
    /// list the files available on the camera
    List {
        /// the hexadecimal file handle of a folder; if provided, the contents
        /// of the folder will be listed
        #[clap(parse(try_from_str = ps_serde_util::parse_hex_u32))]
        parent: Option<u32>,
    },

    /// download a file from the camera
    Get {
        /// the hexadecimal file handle of a file
        #[clap(parse(try_from_str = ps_serde_util::parse_hex_u32))]
        handle: u32,
    },
}

impl FromStr for ExposureMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "m" | "manual" | "manual-exposure" => Ok(ExposureMode::ManualExposure),
            "p" | "program-auto" => Ok(ExposureMode::ProgramAuto),
            "a" | "aperture" | "apeture-priority" => Ok(ExposureMode::AperturePriority),
            "s" | "shutter" | "shutter-priority" => Ok(ExposureMode::ShutterPriority),
            "i" | "intelligent-auto" => Ok(ExposureMode::IntelligentAuto),
            "superior-auto" => Ok(ExposureMode::SuperiorAuto),
            "movie-program-auto" => Ok(ExposureMode::MovieProgramAuto),
            "movie-aperture-priority" => Ok(ExposureMode::MovieAperturePriority),
            "movie-shutter-priority" => Ok(ExposureMode::MovieShutterPriority),
            "movie-manual-exposure" => Ok(ExposureMode::MovieManualExposure),
            "movie-intelligent-auto" => Ok(ExposureMode::MovieIntelligentAuto),
            _ => bail!("invalid camera exposure mode"),
        }
    }
}

impl FromStr for SaveMedia {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "host" | "host-device" => Ok(SaveMedia::HostDevice),
            "cam" | "camera" => Ok(SaveMedia::MemoryCard1),
            _ => bail!("invalid camera save mode"),
        }
    }
}

impl FromStr for ZoomMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "o" | "optical" => Ok(ZoomMode::Optical),
            "od" | "optical-digital" => Ok(ZoomMode::OpticalDigital),
            _ => bail!("invalid camera zoom mode"),
        }
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraCommandContinuousCaptureRequest {
    Start,
    Stop,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CameraCommandRecordRequest {
    Start,
    Stop,
}

impl FromStr for OperatingMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "standby" => Self::Standby,
            "still" | "image" => Self::StillRec,
            "movie" | "video" => Self::MovieRec,
            "transfer" => Self::ContentsTransfer,
            _ => bail!("invalid operating mode"),
        })
    }
}

impl FromStr for FocusMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "manual" | "m" => Self::Manual,
            "afc" => Self::AutoFocusContinuous,
            "afs" => Self::AutoFocusStill,
            _ => bail!("invalid focus mode"),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum CameraResponse {
    Unit,
    Data {
        data: Vec<u8>,
    },
    Download {
        name: String,
    },
    StorageInfo {
        storages: HashMap<ptp::StorageId, ptp::PtpStorageInfo>,
    },
    ObjectInfo {
        objects: HashMap<ptp::ObjectHandle, ptp::PtpObjectInfo>,
    },
    ZoomLevel(u8),
    CcInterval(f32),
    SaveMode(SaveMedia),
    OperatingMode(OperatingMode),
    ExposureMode(ExposureMode),
    FocusMode(FocusMode),
}

macro_rules! get_camera_property {
    ($interface: expr, $prop: ident, $ty: ident) => {
        match $interface.get_value(crate::state::CameraPropertyCode::$prop).await {
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

pub(super) async fn cmd_status(
    iface: &mut CameraInterface,
) -> anyhow::Result<CameraResponse> {
    iface.update().await?;

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
    let error = get_camera_property!(i, Caution, UINT16)?.and_then(ErrorMode::from_u16);

    let shutter_speed =
        get_camera_property!(i, ShutterSpeed, UINT32)?.and_then(ShutterSpeed::from_u32);
    let iso = get_camera_property!(i, ISO, UINT32)?.and_then(Iso::from_u32);
    let aperture = get_camera_property!(i, FNumber, UINT16)?.and_then(Aperture::from_u16);

    println!("operating mode: {operating:?}");
    println!("compression mode: {compression:?}");
    println!("exposure mode: {exposure:?}");
    println!("focus mode: {focus}");
    println!("save media: {save_media:?}");
    println!("error: {error:?}");

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

    Ok(CameraResponse::Unit)
}

pub(super) async fn cmd_get(
    interface: CameraInterfaceRequestBuffer,
    req: CameraCommandGetRequest,
) -> anyhow::Result<CameraResponse> {
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
            ptp::PtpData::UINT16(mode) => CameraResponse::ExposureMode(
                FromPrimitive::from_u16(mode)
                    .context("invalid camera exposure mode (wrong value)")?,
            ),
            _ => bail!("invalid camera exposure mode (wrong data type)"),
        },
        CameraCommandGetRequest::OperatingMode => match prop_info.current {
            ptp::PtpData::UINT8(mode) => CameraResponse::OperatingMode(
                FromPrimitive::from_u8(mode)
                    .context("invalid camera operating mode (wrong value)")?,
            ),
            _ => bail!("invalid camera operating mode (wrong data type)"),
        },
        CameraCommandGetRequest::SaveMode => match prop_info.current {
            ptp::PtpData::UINT16(mode) => CameraResponse::SaveMode(
                FromPrimitive::from_u16(mode).context("invalid camera save mode (wrong value)")?,
            ),
            _ => bail!("invalid camera save mode (wrong data type)"),
        },
        CameraCommandGetRequest::FocusMode => match prop_info.current {
            ptp::PtpData::UINT16(mode) => CameraResponse::FocusMode(
                FromPrimitive::from_u16(mode).context("invalid camera focus mode (wrong value)")?,
            ),
            _ => bail!("invalid camera focus mode (wrong data type)"),
        },
        CameraCommandGetRequest::ZoomLevel => match prop_info.current {
            ptp::PtpData::UINT16(level) => CameraResponse::ZoomLevel(level as u8),
            _ => bail!("invalid camera zoom level (wrong data type)"),
        },
        CameraCommandGetRequest::CcInterval => match prop_info.current {
            ptp::PtpData::UINT16(interval) => {
                CameraResponse::CcInterval(interval as f32 / 10.0)
            }
            _ => bail!("invalid camera zoom level (wrong data type)"),
        },
        CameraCommandGetRequest::Other(_) => todo!(),
    })
}

pub(super) async fn cmd_set(
    interface: CameraInterfaceRequestBuffer,
    req: CameraCommandSetRequest,
) -> anyhow::Result<CameraResponse> {
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

    Ok(CameraResponse::Unit)
}

pub(super) async fn cmd_capture(
    interface: CameraInterfaceRequestBuffer,
    ptp_rx: &mut broadcast::Receiver<ptp::PtpEvent>,
) -> anyhow::Result<CameraResponse> {
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

    Ok(CameraResponse::Unit)
}

pub(super) async fn cmd_continuous_capture(
    interface: CameraInterfaceRequestBuffer,
    req: CameraCommandContinuousCaptureRequest,
) -> anyhow::Result<CameraResponse> {
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

    Ok(CameraResponse::Unit)
}

pub(super) async fn cmd_storage(
    interface: CameraInterfaceRequestBuffer,
    req: CameraCommandStorageRequest,
) -> anyhow::Result<CameraResponse> {
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
                        .map(|storages| CameraResponse::StorageInfo { storages })
                })
                .await
        }
    }
}

pub(super) async fn cmd_file(
    interface: CameraInterfaceRequestBuffer,
    req: CameraCommandFileRequest,
    client_tx: broadcast::Sender<CameraEvent>,
) -> anyhow::Result<CameraResponse> {
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
                    .map(|objects| CameraResponse::ObjectInfo { objects })
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

            let _ = client_tx.send(CameraEvent::Download {
                image_name: info.filename.clone(),
                image_data: Arc::new(data),
                cc_timestamp: None,
            });

            Ok(CameraResponse::Download {
                name: info.filename,
            })
        }
    }
}
