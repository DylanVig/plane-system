use std::{path::PathBuf, sync::Arc};

use anyhow::Context;
use futures::{select, FutureExt};
use tokio::io::AsyncWriteExt;

use crate::{camera::CameraEvent, cli::config::ImageConfig, state::TelemetryInfo, Channels};

#[derive(Clone, Debug)]
pub struct ImageEvent {
    pub data: Arc<Vec<u8>>,
    pub file: PathBuf,
    pub telemetry: Option<TelemetryInfo>,
}

/// This client handles receiving images from the camera, downloading them, and
/// pairing with telemetry.
pub struct ImageClient {
    channels: Arc<Channels>,
    config: ImageConfig,
}

impl ImageClient {
    pub fn new(channels: Arc<Channels>, config: ImageConfig) -> Self {
        ImageClient { channels, config }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let mut interrupt_recv = self.channels.interrupt.subscribe();
        let mut camera_recv = self.channels.camera_event.subscribe();

        let interrupt_fut = interrupt_recv.recv().fuse();

        futures::pin_mut!(interrupt_fut);

        loop {
            select! {
                camera_evt = camera_recv.recv().fuse() => {
                    if let Ok(camera_evt) = camera_evt {
                        match camera_evt {
                            CameraEvent::Download { image_name, image_data, .. } => {
                                debug!("image download detected, uploading file to ground server");

                                let telemetry_info = self.channels.telemetry.borrow().clone();

                                if telemetry_info.is_none() {
                                    warn!("no telemetry data available for image capture")
                                }

                                let image_filename = match self.download_image(&image_name, &image_data, &telemetry_info).await {
                                    Ok(image_filename) => image_filename,
                                    Err(err) => {
                                      warn!("failed to download image: {}", err);
                                      continue;
                                    }
                                };

                                self.channels.image_event.send(ImageEvent {
                                  data: image_data,
                                  file: image_filename,
                                  telemetry: telemetry_info
                                })?;
                            }
                            _ => {}
                        }
                    }
                }
                _ = interrupt_fut => {
                    break;
                }
            }
        }

        Ok(())
    }

    pub async fn download_image(
        &self,
        name: &str,
        image: &Vec<u8>,
        telem: &Option<TelemetryInfo>,
    ) -> anyhow::Result<PathBuf> {
        let mut image_path = self.config.save_path.clone();

        image_path.push(&name);

        debug!("writing image to file '{}'", image_path.to_string_lossy());

        let mut image_file = tokio::fs::File::create(&image_path)
            .await
            .context("failed to create image file")?;

        image_file
            .write_all(&image[..])
            .await
            .context("failed to save image")?;

        info!("wrote image to file '{}'", image_path.to_string_lossy());

        if let Some(telem) = telem {
            let telem_path = image_path.with_extension("json");

            debug!(
                "writing telemetry to file '{}'",
                telem_path.to_string_lossy()
            );

            let mut telem_file = tokio::fs::File::create(telem_path)
                .await
                .context("failed to create telemetry file")?;

            let telem_bytes =
                serde_json::to_vec(telem).context("failed to serialize telemetry to JSON")?;

            telem_file
                .write_all(&telem_bytes[..])
                .await
                .context("failed to write telemetry data to file")?;
        }

        Ok(image_path)
    }
}
