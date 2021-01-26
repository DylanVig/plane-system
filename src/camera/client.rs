use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Context;
use num_traits::{FromPrimitive, ToPrimitive};
use ptp::{ObjectHandle, PtpData, StorageId};
use tokio::{io::AsyncWriteExt, sync::mpsc, time::sleep};

use crate::{util::*, Channels};

use super::interface::*;
use super::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum CameraClientMode {
    Idle,
    ContinuousCapture,
}

pub struct CameraClient {
    iface: CameraInterface,
    channels: Arc<Channels>,
    cmd: mpsc::Receiver<CameraCommand>,
    error: Option<CameraErrorMode>,
    mode: CameraClientMode,
}

impl CameraClient {
    pub fn connect(
        channels: Arc<Channels>,
        cmd: mpsc::Receiver<CameraCommand>,
    ) -> anyhow::Result<Self> {
        let iface = CameraInterface::new().context("failed to create camera interface")?;

        Ok(CameraClient {
            iface,
            channels,
            cmd,
            error: None,
            mode: CameraClientMode::Idle,
        })
    }

    pub fn init(&mut self) -> anyhow::Result<()> {
        trace!("intializing camera");

        self.iface
            .connect()
            .context("error while connecting to camera")?;

        let time_str = chrono::Local::now()
            .format("%Y%m%dT%H%M%S%.3f%:z")
            .to_string();

        trace!("setting time on camera to '{}'", &time_str);

        if let Err(err) = self
            .iface
            .set(CameraPropertyCode::DateTime, PtpData::STR(time_str))
        {
            warn!("could not set date/time on camera: {:?}", err);
        }

        self.iface.update().context("could not get camera state")?;

        info!("initialized camera");

        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.init()?;

        let mut interrupt_recv = self.channels.interrupt.subscribe();

        loop {
            self.iface
                .update()
                .context("failed to update camera state")?;

            match self.cmd.recv().await {
                Some(cmd) => {
                    let result = self.exec(cmd.request()).await;
                    let _ = cmd.respond(result);
                }
                _ => {}
            }

            if let Ok(event) = self.iface.recv() {
                trace!("received event: {:?}", event);

                // in CC mode, if we receive an image capture event we should
                // automatically download the image
                match self.mode {
                    CameraClientMode::ContinuousCapture => match event.code {
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

                                                let image_path = self.download_image(shot_handle).await?;

                                                info!("saved continuous capture image to {:?}", image_path);
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

            if interrupt_recv.try_recv().is_ok() {
                break;
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        info!("disconnecting from camera");
        self.iface.disconnect()?;

        Ok(())
    }

    async fn exec(&mut self, cmd: &CameraRequest) -> anyhow::Result<CameraResponse> {
        match cmd {
            CameraRequest::Reset => {
                let _ = self.iface.disconnect();

                self.iface.reset().context("error while resetting camera")?;

                tokio::time::sleep(Duration::from_secs(3)).await;

                self.iface = CameraInterface::new().context("failed to create camera interface")?;
                self.init()?;
                self.ensure_mode(0x02).await?;

                Ok(CameraResponse::Unit)
            }

            CameraRequest::Storage(cmd) => match cmd {
                CameraStorageRequest::List => {
                    self.ensure_mode(0x04).await?;

                    trace!("getting storage ids");

                    let storage_ids = retry_delay(10, Duration::from_secs(1), || {
                        trace!("checking for storage ID 0x00010000");

                        let storage_ids = self
                            .iface
                            .storage_ids()
                            .context("could not get storage ids")?;

                        if storage_ids.contains(&StorageId::from(0x00010000)) {
                            bail!("no logical storage available");
                        } else {
                            Ok(storage_ids)
                        }
                    })
                    .await?;

                    trace!("got storage ids: {:?}", storage_ids);

                    storage_ids
                        .iter()
                        .map(|&id| self.iface.storage_info(id).map(|info| (id, info)))
                        .collect::<Result<HashMap<_, _>, _>>()
                        .map(|storages| CameraResponse::StorageInfo { storages })
                }
            },

            CameraRequest::File(cmd) => match cmd {
                CameraFileRequest::List { parent } => {
                    self.ensure_mode(0x04).await?;

                    trace!("getting object handles");

                    // wait for storage ID 0x00010001 to exist

                    retry_delay(10, Duration::from_secs(1), || {
                        trace!("checking for storage ID 0x00010001");

                        let storage_ids = self
                            .iface
                            .storage_ids()
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
                        )
                        .context("could not get object handles")?;

                    trace!("got object handles: {:?}", object_handles);

                    object_handles
                        .iter()
                        .map(|&id| self.iface.object_info(id).map(|info| (id, info)))
                        .collect::<Result<HashMap<_, _>, _>>()
                        .map(|objects| CameraResponse::ObjectInfo { objects })
                }

                CameraFileRequest::Get { handle } => {
                    let shot_handle = ObjectHandle::from(*handle);

                    let image_path = self.download_image(shot_handle).await?;

                    Ok(CameraResponse::File { path: image_path })
                }
            },

            CameraRequest::Power(cmd) => {
                self.ensure_mode(0x02).await?;

                match cmd {
                    CameraPowerRequest::Up => self
                        .iface
                        .execute(CameraControlCode::PowerOff, ptp::PtpData::UINT16(1))?,
                    CameraPowerRequest::Down => self
                        .iface
                        .execute(CameraControlCode::PowerOff, ptp::PtpData::UINT16(2))?,
                };

                Ok(CameraResponse::Unit)
            }

            CameraRequest::Reconnect => {
                self.iface
                    .disconnect()
                    .context("error while disconnecting from camera")?;
                self.init().context("error while initializing camera")?;
                self.ensure_mode(0x02).await?;

                Ok(CameraResponse::Unit)
            }

            CameraRequest::Capture => {
                self.ensure_mode(0x02).await?;

                info!("capturing image");

                // press shutter button halfway to fix the focus
                self.iface
                    .execute(CameraControlCode::S1Button, PtpData::UINT16(0x0002))?;

                sleep(Duration::from_millis(200)).await;

                // shoot!
                self.iface
                    .execute(CameraControlCode::S2Button, PtpData::UINT16(0x0002))?;

                sleep(Duration::from_millis(200)).await;

                // release
                self.iface
                    .execute(CameraControlCode::S2Button, PtpData::UINT16(0x0001))?;

                sleep(Duration::from_millis(200)).await;

                // hell yeah
                self.iface
                    .execute(CameraControlCode::S1Button, PtpData::UINT16(0x0001))?;

                info!("waiting for image confirmation");

                tokio::time::timeout(Duration::from_millis(3000), async {
                    loop {
                        trace!("checking for events");

                        if let Ok(event) = self.iface.recv() {
                            // 0xC204 = image taken
                            match event.code {
                                ptp::EventCode::Vendor(0xC204) => match event.params[0] {
                                    Some(1) => break,
                                    Some(2) => bail!("capture failure"),
                                    _ => bail!("unknown capture status"),
                                },
                                evt => trace!("received event: {:?}", evt),
                            }
                        }

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

                let image_path = self.download_image(shot_handle).await?;

                Ok(CameraResponse::File { path: image_path })
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
                        let prop = self
                            .iface
                            .update()
                            .context("failed to query camera properties")?
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
                        let prop = self
                            .iface
                            .update()
                            .context("failed to query camera properties")?
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
                    let prop = self
                        .iface
                        .update()
                        .context("failed to query camera properties")?
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
        retry_delay(10, Duration::from_millis(1000), || {
            trace!("checking operating mode");

            let current_state = self
                .iface
                .update()
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

        retry_delay(10, Duration::from_millis(1000), || {
            debug!("setting {:?} to {:?}", setting, value);

            self.iface
                .set(setting, value.clone())
                .context(format!("failed to set {:?}", setting))?;

            trace!("checking setting {:?}", setting);

            let current_state = self
                .iface
                .update()
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

    async fn download_image(&mut self, handle: ObjectHandle) -> anyhow::Result<PathBuf> {
        let shot_info = self
            .iface
            .object_info(handle)
            .context("error while getting image info")?;

        let shot_data = self
            .iface
            .object_data(handle)
            .context("error while getting image data")?;

        let mut image_path = std::env::current_dir().context("failed to get current directory")?;

        image_path.push(shot_info.filename);

        debug!("writing image to file '{}'", image_path.to_string_lossy());

        let mut image_file = tokio::fs::File::create(&image_path)
            .await
            .context("failed to create file")?;

        image_file
            .write_all(&shot_data[..])
            .await
            .context("failed to save image")?;

        info!("wrote image to file '{}'", image_path.to_string_lossy());

        Ok(image_path)
    }
}
