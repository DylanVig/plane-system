use anyhow::Context;
use chrono::{Datelike, Timelike};
use futures::{FutureExt, StreamExt};
use num_traits::{FromPrimitive, ToPrimitive};
use ptp::{ObjectFormatCode, ObjectHandle, PtpData, StandardObjectFormatCode, StorageId};
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};
use tokio::{io::AsyncWriteExt, time::sleep};

use crate::{state::TelemetryInfo, util::*, Channels};

use super::interface::*;
use super::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum CameraClientMode {
    Idle,
    ContinuousCapture,
}

pub struct CameraClient {
    iface: CameraInterfaceAsync,
    channels: Arc<Channels>,
    cmd: flume::Receiver<CameraCommand>,
    error: Option<CameraErrorMode>,
    mode: CameraClientMode,
}

const TIMEOUT: Duration = Duration::from_secs(1);

impl CameraClient {
    pub fn connect(
        channels: Arc<Channels>,
        cmd: flume::Receiver<CameraCommand>,
    ) -> anyhow::Result<Self> {
        let iface = CameraInterfaceAsync::new().context("failed to create camera interface")?;

        Ok(CameraClient {
            iface,
            channels,
            cmd,
            error: None,
            mode: CameraClientMode::Idle,
        })
    }

    pub async fn init(&mut self) -> anyhow::Result<()> {
        trace!("intializing camera");

        self.iface
            .connect()
            .await
            .context("error while connecting to camera")?;

        let time_str = chrono::Local::now()
            .format("%Y%m%dT%H%M%S%.3f%:z")
            .to_string();

        trace!("setting time on camera to '{}'", &time_str);

        if let Err(err) = self
            .iface
            .set(CameraPropertyCode::DateTime, PtpData::STR(time_str))
            .await
        {
            warn!("could not set date/time on camera: {:?}", err);
        }

        self.iface
            .update()
            .await
            .context("could not get camera state")?;

        info!("initialized camera");

        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.init().await?;

        let mut interrupt_recv = self.channels.interrupt.subscribe();
        let interrupt_fut = interrupt_recv.recv().fuse();
        futures::pin_mut!(interrupt_fut);

        let telemetry_chan = self.channels.telemetry.clone();
        let mut telemetry_stream = tokio_stream::wrappers::WatchStream::new(telemetry_chan).fuse();
        let mut cmd_stream = self.cmd.clone().into_stream().fuse();

        loop {
            self.iface
                .update()
                .await
                .context("failed to update camera state")?;

            futures::select! {
                cmd = cmd_stream.next() => {
                    // this is only None if the command stream closes for some reason
                    let cmd = cmd.unwrap();
                    let request = cmd.request();
                    let result = self.exec(request).await;
                    trace!("command completed, sending response");
                    cmd.respond(result).expect("help");
                }
                telemetry = telemetry_stream.next() => {
                    // this is only None if the telemetry stream closes for some reason
                    let telemetry = telemetry.unwrap();
                    if let Some(telemetry) = telemetry {
                        let camera_data = telemetry_to_camera_data(telemetry);
                        if let Err(err) = self.iface.set(CameraPropertyCode::LocationInfo, camera_data).await {
                            warn!("setting gps location in camera failed: {:?}", err);
                        } else {
                            trace!("sent telemetry info to camera");
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(20)).fuse() => {
                    // if there is no telemetry, we still want to check the
                    // camera for events, so we can't allow this select to block
                    // indefinitely
                }
                _ = &mut interrupt_fut => break,
            }

            if let Ok(cam_evt) = self.iface.recv(Some(TIMEOUT)).await {
                debug!("received event: {:X?}", cam_evt);

                // in CC mode, if we receive an image capture event we should
                // automatically download the image
                match self.mode {
                    CameraClientMode::ContinuousCapture => match cam_evt.code {
                        ptp::EventCode::Vendor(0xC204) => {
                            debug!("received image during continuous capture");

                            let save_media = self
                                .iface
                                .get(CameraPropertyCode::SaveMedia)
                                .context("unknown whether image is saved to host or device")?
                                .current;

                            match save_media {
                                PtpData::UINT16(save_media) => {
                                    match CameraSaveMode::from_u16(save_media) {
                                            Some(save_media) => match save_media {
                                                CameraSaveMode::HostDevice => {
                                                    let shot_handle = ObjectHandle::from(0xFFFFC001);

                                                    let image_name = self.download_image(shot_handle).await?;

                                                    info!("saved continuous capture image to {:?}", image_name);
                                                }

                                                CameraSaveMode::MemoryCard1 => warn!("continuous capture images are being saved to camera; this is not supported"),
                                            },
                                            None => bail!("invalid save media"),
                                        }
                                }
                                _ => bail!("invalid save media"),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }

            if let Err(camera_error) = self.check_error() {
                error!("detected camera error: {:?}", camera_error);
            }
        }

        info!("disconnecting from camera");
        self.iface.disconnect().await?;

        Ok(())
    }

    async fn exec(&mut self, cmd: &CameraRequest) -> anyhow::Result<CameraResponse> {
        match cmd {
            CameraRequest::Reset => {
                let _ = self.iface.disconnect();

                self.iface
                    .reset()
                    .await
                    .context("error while resetting camera")?;

                tokio::time::sleep(Duration::from_secs(3)).await;

                self.iface =
                    CameraInterfaceAsync::new().context("failed to create camera interface")?;
                self.init().await?;
                self.ensure_mode(0x02).await?;

                Ok(CameraResponse::Unit)
            }

            CameraRequest::Debug {
                property,
                value_num,
            } => {
                let current_state = self
                    .iface
                    .update()
                    .await
                    .context("could not get current camera state")?;

                if let Some(property) = property {
                    let property_code: CameraPropertyCode = FromPrimitive::from_u32(*property)
                        .context("not a valid camera property code")?;
                    println!("dumping {:#X?}", property_code);

                    let property = current_state.get(&property_code);
                    println!("dumping {:#X?}", property);

                    if let Some(property) = property {
                        if let Some(&value) = value_num.first() {
                            let property_value = match property.data_type {
                                0x0001 => PtpData::INT8(value as i8),
                                0x0002 => PtpData::UINT8(value as u8),
                                0x0003 => PtpData::INT16(value as i16),
                                0x0004 => PtpData::UINT16(value as u16),
                                0x0005 => PtpData::INT32(value as i32),
                                0x0006 => PtpData::UINT32(value as u32),
                                0x0007 => PtpData::INT64(value as i64),
                                0x0008 => PtpData::UINT64(value as u64),
                                _ => bail!("cannot set this property type, not implemented"),
                            };

                            println!("setting {:#X?} to {:#X?}", property_code, property_value);

                            self.ensure_setting(property_code, property_value).await?;
                        }
                    }
                } else {
                    println!("{:#X?}", current_state);
                }

                Ok(CameraResponse::Unit)
            }

            CameraRequest::Storage(cmd) => match cmd {
                CameraStorageRequest::List => {
                    self.ensure_mode(0x04).await?;

                    debug!("getting storage ids");

                    let storage_ids = retry_async(10, Some(Duration::from_secs(1)), || async {
                        debug!("checking for storage ID 0x00010000");

                        let storage_ids = self
                            .iface
                            .storage_ids(Some(TIMEOUT))
                            .await
                            .context("could not get storage ids")?;

                        if storage_ids.contains(&StorageId::from(0x00010000)) {
                            bail!("no logical storage available");
                        } else {
                            Ok(storage_ids)
                        }
                    })
                    .await?;

                    debug!("got storage ids: {:?}", storage_ids);

                    let infos: Vec<Result<(_, _), _>> =
                        futures::future::join_all(storage_ids.iter().map(|&id| {
                            let iface = &self.iface;
                            async move {
                                iface
                                    .storage_info(id, Some(TIMEOUT))
                                    .await
                                    .map(|info| (id, info))
                            }
                        }))
                        .await;

                    infos
                        .into_iter()
                        .collect::<Result<HashMap<_, _>, _>>()
                        .map(|storages| CameraResponse::StorageInfo { storages })
                }
            },

            CameraRequest::File(cmd) => match cmd {
                CameraFileRequest::List { parent } => {
                    self.ensure_mode(0x04).await?;

                    debug!("getting object handles");

                    // wait for storage ID 0x00010001 to exist

                    retry_async(10, Some(Duration::from_secs(1)), || async {
                        debug!("checking for storage ID 0x00010001");

                        let storage_ids = self
                            .iface
                            .storage_ids(Some(TIMEOUT))
                            .await
                            .context("could not get storage ids")?;

                        if !storage_ids.contains(&StorageId::from(0x00010001)) {
                            bail!("no storage available");
                        } else {
                            Ok(())
                        }
                    })
                    .await?;

                    let object_handles = self
                        .iface
                        .object_handles(
                            ptp::StorageId::from(0x00010001),
                            parent
                                .clone()
                                .map(|v| ObjectHandle::from(v))
                                .or(Some(ptp::ObjectHandle::root())),
                            Some(TIMEOUT),
                        )
                        .await
                        .context("could not get object handles")?;

                    debug!("got object handles: {:?}", object_handles);

                    futures::future::join_all(object_handles.iter().map(|&id| {
                        let iface = &self.iface;
                        async move {
                            iface
                                .object_info(id, Some(TIMEOUT))
                                .await
                                .map(|info| (id, info))
                        }
                    }))
                    .await
                    .into_iter()
                    .collect::<Result<HashMap<_, _>, _>>()
                    .map(|objects| CameraResponse::ObjectInfo { objects })
                }

                CameraFileRequest::Get { handle } => {
                    let shot_handle = ObjectHandle::from(*handle);

                    let image_name = self.download_image(shot_handle).await?;

                    Ok(CameraResponse::Download { name: image_name })
                }
            },

            CameraRequest::Power(cmd) => {
                self.ensure_mode(0x02).await?;

                match cmd {
                    CameraPowerRequest::Up => {
                        self.iface
                            .execute(CameraControlCode::PowerOff, ptp::PtpData::UINT16(1))
                            .await?
                    }
                    CameraPowerRequest::Down => {
                        self.iface
                            .execute(CameraControlCode::PowerOff, ptp::PtpData::UINT16(2))
                            .await?
                    }
                };

                Ok(CameraResponse::Unit)
            }

            CameraRequest::Reconnect => {
                self.iface
                    .disconnect()
                    .await
                    .context("error while disconnecting from camera")?;

                self.init()
                    .await
                    .context("error while initializing camera")?;

                self.ensure_mode(0x02).await?;

                Ok(CameraResponse::Unit)
            }

            CameraRequest::Capture => {
                self.ensure_mode(0x02).await?;

                info!("capturing image");

                let shooting_file_status = self
                    .iface
                    .get(CameraPropertyCode::ShootingFileInfo)
                    .map(|p| match p.current {
                        PtpData::UINT16(u) => u,
                        _ => unreachable!(),
                    })
                    .unwrap_or(0);

                debug!("sending half shutter press");

                // press shutter button halfway to fix the focus
                self.iface
                    .execute(CameraControlCode::S1Button, PtpData::UINT16(0x0002))
                    .await?;

                debug!("sending full shutter press");

                // shoot!
                self.iface
                    .execute(CameraControlCode::S2Button, PtpData::UINT16(0x0002))
                    .await?;

                debug!("sending full shutter release");

                // release
                self.iface
                    .execute(CameraControlCode::S2Button, PtpData::UINT16(0x0001))
                    .await?;

                debug!("sending half shutter release");

                // hell yeah
                self.iface
                    .execute(CameraControlCode::S1Button, PtpData::UINT16(0x0001))
                    .await?;

                info!("waiting for image confirmation");

                tokio::time::timeout(Duration::from_millis(3000), async {
                    loop {
                        let new_props =  self
                        .iface
                        .update()
                        .await
                        ;

                        let new_shooting_file_status =
                            .get(CameraPropertyCode::ShootingFileInfo)
                            .map(|p| match p.current {
                                PtpData::UINT16(u) => u,
                                _ => unreachable!(),
                            })
                            .unwrap_or(0);

                        tokio::task::yield_now().await;
                    }

                    Ok(())
                })
                .await
                .context("timed out while waiting for image confirmation")??;

                info!("received image confirmation");

                let save_media = self
                    .iface
                    .get(CameraPropertyCode::SaveMedia)
                    .context("unknown whether image is saved to host or device")?
                    .current;

                match save_media {
                    PtpData::UINT16(save_media) => match CameraSaveMode::from_u16(save_media) {
                        Some(save_media) => match save_media {
                            // continue
                            CameraSaveMode::HostDevice => {}
                            // we're done here
                            CameraSaveMode::MemoryCard1 => return Ok(CameraResponse::Unit),
                        },
                        None => bail!("invalid save media"),
                    },
                    _ => bail!("invalid save media"),
                }

                let shot_handle = ObjectHandle::from(0xFFFFC001);

                let image_name = self.download_image(shot_handle).await?;

                Ok(CameraResponse::Download { name: image_name })
            }

            CameraRequest::Zoom(req) => match req {
                CameraZoomRequest::Level(req) => match req {
                    CameraZoomLevelRequest::Set { level } => {
                        self.ensure_setting(
                            CameraPropertyCode::ZoomAbsolutePosition,
                            PtpData::UINT16(*level as u16),
                        )
                        .await?;

                        return Ok(CameraResponse::ZoomLevel { zoom_level: *level });
                    }
                    CameraZoomLevelRequest::Get => {
                        let props = self
                            .iface
                            .update()
                            .await
                            .context("failed to query camera properties")?;
                        let prop = props
                            .get(&CameraPropertyCode::ZoomAbsolutePosition)
                            .context("failed to query zoom level")?;

                        if let PtpData::UINT16(level) = prop.current {
                            return Ok(CameraResponse::ZoomLevel {
                                zoom_level: level as u8,
                            });
                        }

                        bail!("invalid zoom level");
                    }
                },
                CameraZoomRequest::Mode(_req) => bail!("unimplemented"),
            },

            CameraRequest::Exposure(req) => match req {
                CameraExposureRequest::Mode(req) => match req {
                    CameraExposureModeRequest::Set { mode } => {
                        self.ensure_setting(
                            CameraPropertyCode::ExposureMode,
                            PtpData::UINT16(mode.to_u16().unwrap()),
                        )
                        .await?;

                        return Ok(CameraResponse::ExposureMode {
                            exposure_mode: *mode,
                        });
                    }
                    CameraExposureModeRequest::Get => {
                        let props = self
                            .iface
                            .update()
                            .await
                            .context("failed to query camera properties")?;

                        let prop = props
                            .get(&CameraPropertyCode::ExposureMode)
                            .context("failed to query exposure mode")?;

                        if let PtpData::UINT16(mode) = prop.current {
                            if let Some(exposure_mode) = CameraExposureMode::from_u16(mode) {
                                return Ok(CameraResponse::ExposureMode { exposure_mode });
                            }
                        }

                        bail!("invalid exposure mode");
                    }
                },
            },

            CameraRequest::SaveMode(req) => match req {
                CameraSaveModeRequest::Set { mode } => {
                    self.ensure_setting(
                        CameraPropertyCode::SaveMedia,
                        PtpData::UINT16(mode.to_u16().unwrap()),
                    )
                    .await?;

                    return Ok(CameraResponse::SaveMode { save_mode: *mode });
                }
                CameraSaveModeRequest::Get => {
                    let props = self
                        .iface
                        .update()
                        .await
                        .context("failed to query camera properties")?;

                    let prop = props
                        .get(&CameraPropertyCode::SaveMedia)
                        .context("failed to query save media")?;

                    if let PtpData::UINT16(mode) = prop.current {
                        if let Some(save_mode) = CameraSaveMode::from_u16(mode) {
                            return Ok(CameraResponse::SaveMode { save_mode });
                        }
                    }

                    bail!("invalid save media");
                }
            },

            CameraRequest::ContinuousCapture(req) => match req {
                CameraContinuousCaptureRequest::Start => {
                    self.iface
                        .execute(
                            CameraControlCode::IntervalStillRecording,
                            PtpData::UINT16(0x0002),
                        )
                        .await
                        .context("failed to start interval recording")?;
                    self.mode = CameraClientMode::ContinuousCapture;

                    Ok(CameraResponse::Unit)
                }
                CameraContinuousCaptureRequest::Stop => {
                    self.iface
                        .execute(
                            CameraControlCode::IntervalStillRecording,
                            PtpData::UINT16(0x0001),
                        )
                        .await
                        .context("failed to stop interval recording")?;

                    self.mode = CameraClientMode::Idle;

                    Ok(CameraResponse::Unit)
                }
                CameraContinuousCaptureRequest::Interval { interval } => {
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

                    self.ensure_setting(
                        CameraPropertyCode::IntervalTime,
                        PtpData::UINT16(interval),
                    )
                    .await
                    .context("failed to set camera interval")?;

                    Ok(CameraResponse::Unit)
                }
            },

            CameraRequest::OperationMode(req) => match req {
                CameraOperationModeRequest::Set { mode } => {
                    self.ensure_setting(
                        CameraPropertyCode::OperatingMode,
                        PtpData::UINT8(mode.to_u8().unwrap()),
                    )
                    .await?;

                    return Ok(CameraResponse::OperatingMode {
                        operating_mode: *mode,
                    });
                }
                CameraOperationModeRequest::Get => {
                    let props = self
                        .iface
                        .update()
                        .await
                        .context("failed to query camera properties")?;

                    let prop = props
                        .get(&CameraPropertyCode::OperatingMode)
                        .context("failed to query operating mode")?;

                    if let PtpData::UINT8(mode) = prop.current {
                        if let Some(operating_mode) = CameraOperatingMode::from_u8(mode) {
                            return Ok(CameraResponse::OperatingMode { operating_mode });
                        }
                    }

                    bail!("invalid operating mode: {:?}", prop.current);
                }
            },

            CameraRequest::Record(req) => match req {
                CameraRecordRequest::Start => {
                    self.iface
                        .execute(CameraControlCode::MovieRecording, PtpData::UINT16(0x0002))
                        .await?;

                    return Ok(CameraResponse::Unit);
                }
                CameraRecordRequest::Stop => {
                    self.iface
                        .execute(CameraControlCode::MovieRecording, PtpData::UINT16(0x0001))
                        .await?;

                    return Ok(CameraResponse::Unit);
                }
            },

            CameraRequest::FocusMode(req) => match req {
                CameraFocusModeRequest::Set { mode } => {
                    self.ensure_setting(
                        CameraPropertyCode::FocusMode,
                        PtpData::UINT16(mode.to_u16().unwrap()),
                    )
                    .await?;

                    return Ok(CameraResponse::FocusMode { focus_mode: *mode });
                }
                CameraFocusModeRequest::Get => {
                    let props = self
                        .iface
                        .update()
                        .await
                        .context("failed to query camera properties")?;

                    let prop = props
                        .get(&CameraPropertyCode::FocusMode)
                        .context("failed to query focus mode")?;

                    if let PtpData::UINT16(mode) = prop.current {
                        if let Some(focus_mode) = CameraFocusMode::from_u16(mode) {
                            return Ok(CameraResponse::FocusMode { focus_mode });
                        }
                    }

                    bail!("invalid operating mode: {:?}", prop.current);
                }
            },
        }
    }

    /// Checks if the camera registers a new error. Will return a given error
    /// only once, and then returns Ok until the error changes.
    fn check_error(&mut self) -> Result<(), CameraErrorMode> {
        let caution_prop = self.iface.get(CameraPropertyCode::Caution);

        if let Some(caution_prop) = caution_prop {
            if let PtpData::UINT16(caution_value) = caution_prop.current {
                if caution_value != 0x0000 {
                    match CameraErrorMode::from_u16(caution_value) {
                        Some(caution_mode) => {
                            let already_reported = if let Some(current_caution_mode) = self.error {
                                current_caution_mode == caution_mode
                            } else {
                                false
                            };

                            if !already_reported {
                                self.error = Some(caution_mode);
                                return Err(caution_mode);
                            }
                        }
                        None => {
                            warn!(
                                "encountered unknown camera error status: 0x{:04x}",
                                caution_value
                            );
                        }
                    }
                }
            }
        }

        self.error = None;

        Ok(())
    }

    async fn ensure_mode(&mut self, mode: u8) -> anyhow::Result<()> {
        retry_async(10, Some(Duration::from_millis(1000)), || async {
            trace!("checking operating mode");

            let current_state = self
                .iface
                .update()
                .await
                .context("could not get current camera state")?;

            let current_op_mode = current_state.get(&CameraPropertyCode::OperatingMode);

            trace!("current op mode: {:?}", current_op_mode);

            if let Some(PtpData::UINT8(current_op_mode)) = current_op_mode.map(|d| &d.current) {
                if *current_op_mode == mode {
                    // we are in the right mode, break
                    return Ok(());
                }
            }

            debug!("setting operating mode to 0x{:04x}", mode);

            self.iface
                .set(CameraPropertyCode::OperatingMode, PtpData::UINT8(mode))
                .await
                .context("failed to set operating mode of camera")?;

            bail!("wrong operating mode")
        })
        .await
    }

    async fn ensure_setting(
        &mut self,
        setting: CameraPropertyCode,
        value: PtpData,
    ) -> anyhow::Result<()> {
        let current_setting = self.iface.get(setting);

        trace!("current {:?}: {:?}", setting, current_setting);

        if let Some(current_setting) = current_setting {
            if current_setting.current == value {
                // we are in the right mode, break
                return Ok(());
            }

            if current_setting.is_enable != 1 || current_setting.get_set != 1 {
                bail!("changing this property is not supported");
            }
        }

        retry_async(10, Some(Duration::from_millis(1000)), || async {
            debug!("setting {:?} to {:?}", setting, value);

            self.iface
                .set(setting, value.clone())
                .await
                .context(format!("failed to set {:?}", setting))?;

            trace!("checking setting {:?}", setting);

            let current_state = self
                .iface
                .update()
                .await
                .context("could not get current camera state")?;

            let current_setting = current_state.get(&setting);

            trace!("current {:?}: {:?}", setting, current_setting);

            if let Some(current_setting) = current_setting {
                if current_setting.current == value {
                    // we are in the right mode, break
                    return Ok(());
                }
            }

            bail!("failed to set {:?}", setting);
        })
        .await
    }

    async fn download_image(&mut self, handle: ObjectHandle) -> anyhow::Result<String> {
        let shot_info = self
            .iface
            .object_info(handle, Some(TIMEOUT))
            .await
            .context("error while getting image info")?;

        let shot_data = self
            .iface
            .object_data(handle, Some(TIMEOUT))
            .await
            .context("error while getting image data")?;

        let image_name = shot_info.filename;

        let _ = self.channels.camera_event.send(CameraEvent::Download {
            image_name: image_name.clone(),
            image_data: Arc::new(shot_data),
        });

        Ok(image_name)
    }
}




fn telemetry_to_camera_data(telemetry: TelemetryInfo) -> PtpData {
    let lat = telemetry.position.latitude.abs();
    let lat_north = telemetry.position.latitude < 0.0;
    let lat_degrees = lat as u32;
    let lat_minutes = (lat * 60.0 % 60.0) as u32;
    let lat_seconds_den = 100000u32;
    let lat_seconds_num = (lat * 3600.0 % 60.0 * lat_seconds_den as f32) as u32;

    let lon = telemetry.position.longitude.abs();
    let lon_west = telemetry.position.longitude < 0.0;
    let lon_degrees = lon as u32;
    let lon_minutes = (lon * 60.0 % 60.0) as u32;
    let lon_seconds_den = 100000u32;
    let lon_seconds_num = (lon * 3600.0 % 60.0 * lon_seconds_den as f32) as u32;

    PtpData::AUINT32(vec![
        0x01, // Info received from a GPS
        0x01, // 3D position data
        if lat_north { 0x00 } else { 0x01 },
        lat_degrees,
        lat_minutes,
        lat_seconds_num,
        lat_seconds_den,
        if lon_west { 0x00 } else { 0x01 },
        lon_degrees,
        lon_minutes,
        lon_seconds_num,
        lon_seconds_den,
        0x01, // relative altitude is included
        if telemetry.position.altitude_rel >= 0.0 {
            0x00
        } else {
            0x01
        },
        (telemetry.position.altitude_rel.abs() * 1000.0) as u32,
        1000, // denominator of altitude is 1000
        0x00, // geoid altitude is not included,
        0x00,
        0,
        0,
        0x00, // VDoP is not included
        0,
        0,
        0x00, // HDoP is not included,
        0,
        0,
        0x00, // PDoP is not included
        0,
        0,
        0x01, // coordinate system is WGS84
        0x00, // speed is not included
        0,
        0,
        0,
        0x00, // true bearing is not included
        0,
        0,
        0x00, // magnetic bearing is not included
        0,
        0,
        telemetry.time.year() as u32,
        telemetry.time.month(),
        telemetry.time.day(),
        telemetry.time.hour(),
        telemetry.time.minute(),
        telemetry.time.second(),
    ])
}
