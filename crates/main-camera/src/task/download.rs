use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use async_trait::async_trait;
use bytes::Bytes;
use log::*;

use ps_client::Task;
use ps_telemetry::Telemetry;
use tokio::{
    fs::File,
    io::AsyncWriteExt,
    select,
    sync::{watch, RwLock},
    time::sleep,
};
use tokio_util::sync::CancellationToken;

use crate::{interface::PropertyCode, task::util::convert_camera_value, DownloadConfig};

use super::InterfaceGuard;

/// This is the object handle for images stored in the image buffer. The image
/// buffer is used when the camera does not contain an SD card, and is used to
/// retrieve images that are stored temporarily on the camera after capture.
const IMAGE_BUFFER_OBJECT_HANDLE: u32 = 0xFFFFC001;

#[derive(Clone, Debug)]
pub struct Download {
    telemetry: Telemetry,
    metadata: ptp::ObjectInfo,
    data: Bytes,
}

pub struct DownloadTask {
    config: DownloadConfig,
    interface: Arc<RwLock<InterfaceGuard>>,

    telem_rx: watch::Receiver<Telemetry>,
    ptp_evt_rx: flume::Receiver<ptp::Event>,
    download_tx: flume::Sender<Download>,
    download_rx: flume::Receiver<Download>,
}

impl DownloadTask {
    pub(super) fn new(
        config: DownloadConfig,
        interface: Arc<RwLock<InterfaceGuard>>,
        telem_rx: watch::Receiver<Telemetry>,
        ptp_evt_rx: flume::Receiver<ptp::Event>,
    ) -> Self {
        let (download_tx, download_rx) = flume::bounded(256);

        Self {
            config,
            interface,
            telem_rx,
            ptp_evt_rx,
            download_rx,
            download_tx,
        }
    }

    pub fn download(&self) -> flume::Receiver<Download> {
        self.download_rx.clone()
    }
}

#[async_trait]
impl Task for DownloadTask {
    fn name(&self) -> &'static str {
        "main-camera/download"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            config,
            interface,
            telem_rx,
            ptp_evt_rx,
            download_tx,
            ..
        } = *self;

        let mut save_path = config.save_path;

        // save inside of a folder named after the current date and time
        save_path.push(chrono::Local::now().format("%FT%H-%M-%S").to_string());

        if let Err(err) = tokio::fs::create_dir_all(&save_path).await {
            anyhow::bail!(
                "could not create image save directory {}: {}",
                save_path.display(),
                err
            );
        }

        let loop_fut = async move {
            loop {
                // wait for shutter event from camera
                loop {
                    match ptp_evt_rx.recv_async().await {
                        Ok(evt) => {
                            if let ptp::EventCode::Vendor(0xC204) | ptp::EventCode::Vendor(0xC203) =
                                evt.code
                            {
                                break;
                            }
                        }
                        Err(err) => {
                            warn!("failed to received event from camera: {err:?}");
                        }
                    }
                }

                let telem = telem_rx.borrow().clone();

                debug!("received capture event from camera");

                // most significant bit indicates that the image is still being
                // acquired, so wait for it to flip to zero
                // MAYBE WRONG? ^
                loop {
                    sleep(Duration::from_millis(100)).await;

                    let props = interface
                        .write()
                        .await
                        .query()
                        .context("could not get camera state")?;
                    let shooting_file_info: u16 =
                        convert_camera_value(&props, PropertyCode::ShootingFileInfo)
                            .context("could not get shooting file info")?;

                    if shooting_file_info & 0x8000 != 0 {
                        break;
                    }
                }

                // download images until ShootingFileInfo reaches zero
                loop {
                    sleep(Duration::from_millis(100)).await;

                    let props = interface
                        .write()
                        .await
                        .query()
                        .context("could not get camera state")?;
                    let shooting_file_info: u16 =
                        convert_camera_value(&props, PropertyCode::ShootingFileInfo)
                            .context("could not get shooting file info")?;

                    if shooting_file_info == 0 {
                        break;
                    }

                    let (metadata, data) = {
                        let mut interface = interface.write().await;

                        tokio::task::block_in_place(|| {
                            let handle = ptp::ObjectHandle::from(IMAGE_BUFFER_OBJECT_HANDLE);
                            let info = interface
                                .get_object_info(handle, None)
                                .context("failed to get info for image")?;
                            let data = interface
                                .get_object(handle, None)
                                .context("failed to get data for image")?;
                            let data = Bytes::from(data);

                            Ok::<_, anyhow::Error>((info, data))
                        })?
                    };

                    info!("downloaded image information from camera");

                    let download = Download {
                        telemetry: telem.clone(),
                        metadata,
                        data,
                    };

                    let _ = download_tx.try_send(download.clone());

                    save(&save_path, download).await?;
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

async fn save(image_save_dir: impl AsRef<Path>, download: Download) -> anyhow::Result<PathBuf> {
    let mut image_path = image_save_dir.as_ref().to_owned();
    image_path.push(&download.metadata.filename);

    debug!("writing image to file '{}'", image_path.to_string_lossy());

    let mut image_file = File::create(&image_path)
        .await
        .context("failed to create image file")?;

    image_file
        .write_all(&download.data[..])
        .await
        .context("failed to save image")?;

    info!("wrote image to file '{}'", image_path.to_string_lossy());

    let telem_path = image_path.with_extension("json");

    debug!(
        "writing telemetry to file '{}'",
        telem_path.to_string_lossy()
    );

    let telem_json = serde_json::json!(&download.telemetry);

    let telem_bytes =
        serde_json::to_vec(&telem_json).context("failed to serialize telemetry to JSON")?;

    let mut telem_file = File::create(telem_path)
        .await
        .context("failed to create telemetry file")?;

    telem_file
        .write_all(&telem_bytes[..])
        .await
        .context("failed to write telemetry data to file")?;

    Ok(image_path)
}
