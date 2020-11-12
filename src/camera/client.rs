use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Context;
use cli_table::{Cell, Row, Table};
use humansize::{file_size_opts, FileSize};
use ptp::{ObjectHandle, PtpData};
use tokio::{
    io::AsyncWriteExt,
    sync::broadcast::{self, TryRecvError},
    time::delay_for,
};

use crate::{command::*, util::*, Channels};

use super::{
    interface::CameraInterface, interface::SonyDeviceControlCode, interface::SonyDevicePropertyCode,
};

pub struct CameraClient {
    iface: CameraInterface,
    channels: Arc<Channels>,
    cmd: broadcast::Receiver<Command>,
    interrupt: broadcast::Receiver<()>,
}

impl CameraClient {
    pub fn connect(channels: Arc<Channels>) -> anyhow::Result<Self> {
        let iface = CameraInterface::new().context("failed to create camera interface")?;

        let cmd = channels.cmd.subscribe();
        let interrupt = channels.interrupt.subscribe();

        Ok(CameraClient {
            iface,
            channels,
            cmd,
            interrupt,
        })
    }

    pub fn init(&mut self) -> anyhow::Result<()> {
        trace!("intializing camera");

        self.iface.connect()?;

        // RFC 3339 = ISO 8601 = camera datetime format
        let time_str = chrono::Local::now().to_rfc3339();

        trace!("setting time on camera to '{}'", &time_str);

        if let Err(err) = self
            .iface
            .set(SonyDevicePropertyCode::DateTime, PtpData::STR(time_str))
        {
            warn!("could not set date/time on camera: {:?}", err);
        }

        info!("initialized camera");

        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.init()?;

        loop {
            match self.interrupt.try_recv() {
                Ok(_) => break,
                Err(TryRecvError::Empty) => {}
                Err(_) => todo!("handle interrupt receiver lagging??"),
            }

            match self.cmd.try_recv() {
                Ok(cmd) => match cmd.data {
                    CommandData::Camera(camera_cmd) => {
                        let result = self.exec(camera_cmd).await;
                        self.channels
                            .response
                            .send(Response::result_for(&cmd, result));
                    }
                    CommandData::Exit => break,
                },
                _ => {}
            }

            if let Ok(event) = self.iface.recv() {
                trace!("received event: {:?}", event);
            }

            tokio::time::delay_for(Duration::from_secs(1)).await;
        }

        info!("disconnecting from camera");
        self.iface.disconnect()?;

        Ok(())
    }

    async fn exec(&mut self, cmd: CameraCommand) -> anyhow::Result<ResponseData> {
        match cmd {
            CameraCommand::Storage(cmd) => match cmd {
                CameraStorageCommand::List => {
                    self.ensure_mode(0x04).await?;

                    trace!("getting storage ids");

                    let storage_ids = self
                        .iface
                        .storage_ids()
                        .context("could not get storage ids")?;

                    trace!("got storage ids: {:?}", storage_ids);

                    storage_ids
                        .iter()
                        .map(|&id| self.iface.storage_info(id).map(|info| (id, info)))
                        .collect::<Result<HashMap<_, _>, _>>()
                        .map(|storages| ResponseData::CameraStorageInfo { storages })
                }
            },
            CameraCommand::File(cmd) => match cmd {
                CameraFileCommand::List => {
                    self.ensure_mode(0x04).await?;

                    trace!("getting object handles");

                    // TODO: wait until camera reports storage id 0x00010001 as
                    // existing

                    let object_handles = self
                        .iface
                        .object_handles(
                            ptp::StorageId::from(0x00010001),
                            Some(ptp::ObjectHandle::root()),
                        )
                        .context("could not get object handles")?;

                    trace!("got object handles: {:?}", object_handles);

                    object_handles
                        .iter()
                        .map(|&id| self.iface.object_info(id).map(|info| (id, info)))
                        .collect::<Result<HashMap<_, _>, _>>()
                        .map(|objects| ResponseData::CameraObjectInfo { objects })
                }
            },
            CameraCommand::Power(cmd) => {
                self.ensure_mode(0x02).await?;

                match cmd {
                    CameraPowerCommand::Up => self
                        .iface
                        .execute(SonyDeviceControlCode::PowerOff, ptp::PtpData::UINT16(1))?,
                    CameraPowerCommand::Down => self
                        .iface
                        .execute(SonyDeviceControlCode::PowerOff, ptp::PtpData::UINT16(2))?,
                };

                Ok(ResponseData::Unit)
            }
            CameraCommand::Reconnect => {
                self.iface = CameraInterface::new().context("failed to create camera interface")?;
                self.init()?;

                Ok(ResponseData::Unit)
            }
            CameraCommand::Capture => {
                self.ensure_mode(0x02).await?;

                // press shutter button halfway to fix the focus
                self.iface
                    .execute(SonyDeviceControlCode::S1Button, PtpData::UINT16(0x0002))?;

                delay_for(Duration::from_millis(100)).await;

                // shoot!
                self.iface
                    .execute(SonyDeviceControlCode::S2Button, PtpData::UINT16(0x0002))?;

                delay_for(Duration::from_millis(100)).await;

                // release
                self.iface
                    .execute(SonyDeviceControlCode::S2Button, PtpData::UINT16(0x0001))?;

                delay_for(Duration::from_millis(100)).await;

                // hell yeah
                self.iface
                    .execute(SonyDeviceControlCode::S1Button, PtpData::UINT16(0x0001))?;

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

                let shot_handle = ObjectHandle::from(0xFFFFC001);

                let shot_info = self
                    .iface
                    .object_info(shot_handle)
                    .context("error while getting shot info")?;

                let shot_data = self
                    .iface
                    .object_data(shot_handle)
                    .context("error while getting shot data")?;

                info!("captured image: {:?}", shot_info);

                info!(
                    "image size: {}",
                    shot_data
                        .len()
                        .file_size(humansize::file_size_opts::BINARY)
                        .unwrap()
                );

                let mut image_path =
                    std::env::current_dir().context("failed to get current directory")?;

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

                Ok(ResponseData::File { path: image_path })
            }
            _ => bail!("not implemented"),
        }
    }

    async fn ensure_mode(&mut self, mode: u8) -> anyhow::Result<()> {
        retry_delay(10, Some(Duration::from_millis(1000)), || {
            trace!("checking operating mode");

            let current_state = self
                .iface
                .update()
                .context("could not get current camera state")?;

            let current_op_mode = current_state.get(&SonyDevicePropertyCode::OperatingMode);

            debug!("current op mode: {:?}", current_op_mode);

            if let Some(PtpData::UINT8(current_op_mode)) = current_op_mode.map(|d| &d.current) {
                if *current_op_mode == mode {
                    // we are in the right mode, break
                    return Ok(());
                }
            }

            debug!("setting operating mode to {:04x}", mode);

            self.iface
                .set(SonyDevicePropertyCode::OperatingMode, PtpData::UINT8(mode))
                .context("failed to set operating mode of camera")?;

            Ok(())
        })
        .await
    }
}
