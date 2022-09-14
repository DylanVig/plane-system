use std::{sync::Arc, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use log::*;
use num_traits::{FromPrimitive, ToPrimitive};
use ps_client::Task;
use ptp::PtpEvent;
use tokio::{
    select,
    sync::RwLock,
};
use tokio_util::sync::CancellationToken;

use crate::{
    interface::{CameraInterface, PropertyCode},
    task::util::{convert_camera_value, get_camera_values},
};

/// This is the object handle for images stored in the image buffer. The image
/// buffer is used when the camera does not contain an SD card, and is used to
/// retrieve images that are stored temporarily on the camera after capture.
const IMAGE_BUFFER_OBJECT_HANDLE: u32 = 0xFFFFC001;

pub type Download = Arc<(ptp::PtpObjectInfo, Vec<u8>)>;

pub struct DownloadTask {
    pub(super) interface: Arc<RwLock<CameraInterface>>,

    pub(super) evt_rx: flume::Receiver<PtpEvent>,
    pub(super) download_tx: flume::Sender<Download>,
}

#[async_trait]
impl Task for DownloadTask {
    fn name() -> &'static str {
        "main-camera/download"
    }

    async fn run(self, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            interface,
            evt_rx,
            download_tx,
        } = self;

        let loop_fut = async move {
            loop {
                // wait for shutter event from camera
                loop {
                    match evt_rx.recv_async().await {
                        Ok(evt) => {
                            if evt.code.to_u16().unwrap() == 0xC204 {
                                break;
                            }
                        }
                        Err(err) => {
                            warn!("failed to received event from camera: {err:?}");
                        }
                    }
                }

                let timestamp = chrono::Local::now();

                debug!("received capture event from camera");

                // most significant bit indicates that the image is still being
                // acquired, so wait for it to flip to zero
                loop {
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    let props = get_camera_values(&*interface)
                        .await
                        .context("could not get camera state")?;
                    let shooting_file_info: u16 =
                        convert_camera_value(&props, PropertyCode::ShootingFileInfo)
                            .context("could not get shooting file info")?;

                    if shooting_file_info & 0x8000 == 0 {
                        break;
                    }
                }

                // download images until ShootingFileInfo reaches zero
                loop {
                    tokio::time::sleep(Duration::from_millis(100)).await;

                    let props = get_camera_values(&*interface)
                        .await
                        .context("could not get camera state")?;
                    let shooting_file_info: u16 =
                        convert_camera_value(&props, PropertyCode::ShootingFileInfo)
                            .context("could not get shooting file info")?;

                    if shooting_file_info == 0 {
                        break;
                    }

                    let (info, data) = {
                        let interface = interface.write().await;

                        tokio::task::block_in_place(|| {
                            let handle = ptp::ObjectHandle::from(IMAGE_BUFFER_OBJECT_HANDLE);
                            let info = interface
                                .object_info(handle, None)
                                .context("failed to get info for image")?;
                            let data = interface
                                .object_data(handle, None)
                                .context("failed to get data for image")?;

                            Ok::<_, anyhow::Error>((info, data))
                        })?
                    };

                    download_tx.send_async(Arc::new((info, data))).await;
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
