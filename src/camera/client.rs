use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use anyhow::Context;
use humansize::{file_size_opts, FileSize};
use num_traits::{FromPrimitive, ToPrimitive};
use ptp::{ObjectHandle, PtpData, StorageId};
use tokio::{io::AsyncWriteExt, sync::mpsc, time::delay_for};

use crate::{util::*, Channels};

use super::interface::*;
use super::*;

pub struct CameraClient {
    iface: CameraInterface,
    channels: Arc<Channels>,
    cmd: mpsc::Receiver<CameraCommand>,
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
        })
    }

    pub fn init(&mut self) -> anyhow::Result<()> {
        trace!("intializing camera");

        self.iface.connect()?;

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

        info!("initialized camera");

        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.init()?;

        let interrupt = self.channels.interrupt.clone();

        loop {
            match self.cmd.try_recv() {
                Ok(cmd) => {
                    let result = self.exec(cmd.request()).await;
                    let _ = cmd.respond(result);
                }
                _ => {}
            }

            if let Ok(event) = self.iface.recv() {
                trace!("received event: {:?}", event);
            }

            if *interrupt.borrow() {
                break;
            }

            tokio::time::delay_for(Duration::from_secs(1)).await;
        }

        info!("disconnecting from camera");
        self.iface.disconnect()?;

        Ok(())
    }

    async fn exec(&mut self, cmd: &CameraRequest) -> anyhow::Result<CameraResponse> {
        match cmd {
            CameraRequest::Reset => {
                self.iface.reset().context("error while resetting camera")?;

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

                    let image_path = self.download_object(shot_handle).await?;

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
                self.iface = CameraInterface::new().context("failed to create camera interface")?;
                self.init()?;

                Ok(CameraResponse::Unit)
            }

            CameraRequest::Capture => {
                self.ensure_mode(0x02).await?;

                // press shutter button halfway to fix the focus
                self.iface
                    .execute(CameraControlCode::S1Button, PtpData::UINT16(0x0002))?;

                delay_for(Duration::from_millis(100)).await;

                // shoot!
                self.iface
                    .execute(CameraControlCode::S2Button, PtpData::UINT16(0x0002))?;

                delay_for(Duration::from_millis(100)).await;

                // release
                self.iface
                    .execute(CameraControlCode::S2Button, PtpData::UINT16(0x0001))?;

                delay_for(Duration::from_millis(100)).await;

                // hell yeah
                self.iface
                    .execute(CameraControlCode::S1Button, PtpData::UINT16(0x0001))?;

                info!("waiting for image event");

                loop {
                    if let Ok(event) = self.iface.recv() {
                        // 0xC204 = image taken
                        match event.code {
                            ptp::EventCode::Vendor(0xC204) => match event.params[0] {
                                Some(1) => break,
                                Some(2) => bail!("capture failure"),
                                _ => bail!("unknown capture status"),
                            },
                            _ => {}
                        }
                    }

                    delay_for(Duration::from_millis(100)).await;
                }

                info!("received image event");

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

                let image_path = self.download_object(shot_handle).await?;

                Ok(CameraResponse::File { path: image_path })
            }

            CameraRequest::Zoom(req) => match req {
                CameraZoomRequest::Level(req) => {
                    if let CameraZoomLevelRequest::Set { level } = req {
                        self.iface
                            .set(
                                CameraPropertyCode::ZoomAbsolutePosition,
                                PtpData::UINT16(*level as u16),
                            )
                            .context("failed to set zoom level")?;
                    };

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
                CameraZoomRequest::Mode(req) => bail!("unimplemented"),
            },

            CameraRequest::Exposure(req) => match req {
                CameraExposureRequest::Mode(req) => {
                    if let CameraExposureModeRequest::Set { mode } = req {
                        self.iface
                            .set(
                                CameraPropertyCode::ExposureMode,
                                PtpData::UINT16(mode.to_u16().unwrap()),
                            )
                            .context("failed to set exposure mode")?;
                    };

                    let prop = self
                        .iface
                        .update()
                        .context("failed to query camera properties")?
                        .get(&CameraPropertyCode::ExposureMode)
                        .context("failed to query save media")?;

                    if let PtpData::UINT16(mode) = prop.current {
                        if let Some(exposure_mode) = CameraExposureMode::from_u16(mode) {
                            return Ok(CameraResponse::ExposureMode { exposure_mode });
                        }
                    }

                    bail!("invalid exposure mode");
                }
            },

            CameraRequest::SaveMode(req) => {
                if let CameraSaveModeRequest::Set { mode } = req {
                    self.iface
                        .set(
                            CameraPropertyCode::SaveMedia,
                            PtpData::UINT16(mode.to_u16().unwrap()),
                        )
                        .context("failed to set save media")?;
                };

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
        }
    }

    async fn ensure_mode(&mut self, mode: u8) -> anyhow::Result<()> {
        retry_delay(10, Duration::from_millis(1000), || {
            trace!("checking operating mode");

            let current_state = self
                .iface
                .update()
                .context("could not get current camera state")?;

            let current_op_mode = current_state.get(&CameraPropertyCode::OperatingMode);

            debug!("current op mode: {:?}", current_op_mode);

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

    async fn download_object(&mut self, handle: ObjectHandle) -> anyhow::Result<PathBuf> {
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

        info!("writing image to file '{}'", image_path.to_string_lossy());

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
