use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use async_trait::async_trait;
use log::{debug, info, warn};
use ps_client::Task;
use ps_telemetry::Telemetry;
use tokio::{fs::File, io::AsyncWriteExt, select, sync::watch};
use tokio_util::sync::CancellationToken;

use crate::config::DownloadConfig;

pub struct DownloadTask {
    config: DownloadConfig,
    telem_rx: watch::Receiver<Telemetry>,
    camera_download_rx: flume::Receiver<ps_main_camera::Download>,
}

pub fn create_task(
    config: DownloadConfig,
    telem_rx: watch::Receiver<Telemetry>,
    camera_download_rx: flume::Receiver<ps_main_camera::Download>,
) -> anyhow::Result<DownloadTask> {
    Ok(DownloadTask {
        config,
        telem_rx,
        camera_download_rx,
    })
}

#[async_trait]
impl Task for DownloadTask {
    fn name(&self) -> &'static str {
        "download"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            config,
            telem_rx,
            camera_download_rx,
        } = *self;

        let DownloadConfig { mut save_path } = config;
        // save inside of a folder named after the current date and time
        save_path.push(chrono::Local::now().format("%FT%H-%M-%S").to_string());

        if let Err(err) = tokio::fs::create_dir_all(&save_path).await {
            bail!(
                "could not create image save directory {}: {}",
                save_path.display(),
                err
            );
        }

        let loop_fut = async move {
            loop {
                let download = camera_download_rx.recv_async().await?;
                let (image_metadata, image_data) = &*download;
                let telem = telem_rx.borrow().clone();

                debug!("image download detected, uploading file to ground server");

                if telem.pixhawk.is_none() {
                    warn!("no pixhawk telemetry data available for image capture")
                }

                // let offset_position = match telem {
                //     Telemetry { pixhawk: Some(pixhawk_telem), csb: Some(csb_telem) } => {
                //         // velocity in meters per second east and north
                //         let (ph_velocity, ph_time) = pixhawk_telem.velocity;
                //         let delay = csb_telem.timestamp - ph_time;
                //         let delay_seconds = delay.num_milliseconds() as f32 / 1000.;

                //         // angle we are traveling at
                //         let heading = pixhawk_telem.attitude.0.yaw;

                //         // distance traveled since we received gps from pixhawk
                //         let distance_xy = f32::sqrt(vx * vx + vy * vy) * delay_seconds;
                //         let offset_coords = pixhawk_telemetry
                //             .position
                //             .point
                //             .haversine_destination(heading, distance_xy);
                //         let offset_altitude_rel =
                //             pixhawk_telemetry.position.altitude_rel + vz * delay_seconds;
                //         let offset_altitude_msl =
                //             pixhawk_telemetry.position.altitude_msl + vz * delay_seconds;

                //         Some(Point3D {
                //             point: offset_coords,
                //             altitude_msl: offset_altitude_msl,
                //             altitude_rel: offset_altitude_rel,
                //         })
                //     }
                //     _ => None,
                // };

                if let Err(err) = save(
                    &save_path,
                    &image_metadata.filename,
                    &image_data,
                    &Some(telem),
                )
                .await
                {
                    warn!("failed to download image: {}", err);
                };
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

async fn save(
    image_save_dir: impl AsRef<Path>,
    image_name: &str,
    image_data: &[u8],
    telemetry: &Option<Telemetry>,
) -> anyhow::Result<PathBuf> {
    let mut image_path = image_save_dir.as_ref().to_owned();
    image_path.push(&image_name);

    debug!("writing image to file '{}'", image_path.to_string_lossy());

    let mut image_file = File::create(&image_path)
        .await
        .context("failed to create image file")?;

    image_file
        .write_all(&image_data[..])
        .await
        .context("failed to save image")?;

    info!("wrote image to file '{}'", image_path.to_string_lossy());

    let telem_path = image_path.with_extension("json");

    debug!(
        "writing telemetry to file '{}'",
        telem_path.to_string_lossy()
    );

    let telem_json = serde_json::json!(telemetry);

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
