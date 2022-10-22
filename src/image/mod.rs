use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use futures::{select, FutureExt};
use tokio::{fs::File, io::AsyncWriteExt};

#[cfg(feature = "csb")]
use crate::camera::main::csb;

use crate::{
    camera::main::{csb, CameraEvent},
    cli::config::ImageConfig,
    state::{Point3D, Telemetry},
    util::ISO_8601_FORMAT,
    Channels,
};

#[derive(Clone, Debug)]
pub struct ImageClientEvent {
    pub data: Arc<Vec<u8>>,
    pub file: PathBuf,
    pub telemetry: Option<Telemetry>,
}

pub async fn run(channels: Arc<Channels>, config: ImageConfig) -> anyhow::Result<()> {
    let mut interrupt_recv = channels.interrupt.subscribe();
    let mut camera_recv = channels.camera_event.subscribe();

    let interrupt_fut = interrupt_recv.recv().fuse();

    futures::pin_mut!(interrupt_fut);

    let mut image_save_dir = config.save_path.clone();
    image_save_dir.push(chrono::Local::now().format("%F_%H-%M-%S").to_string());

    if let Err(err) = tokio::fs::create_dir_all(&image_save_dir).await {
        warn!("could not create image save directory: {}", err);
    }

    loop {19
        select! {
            camera_evt = camera_recv.recv().fuse() => {
                if let Ok(camera_evt) = camera_evt {
                    match camera_evt {
                        CameraEvent::Download { image_name, image_data, cc_timestamp, .. } => {
                            debug!("image download detected, uploading file to ground server");

                            let pixhawk_telemetry = channels.pixhawk_telemetry.borrow().clone();

                            if pixhawk_telemetry.is_none() {
                                warn!("no pixhawk telemetry data available for image capture")
                            }

                            #[cfg(feature = "csb")]
                            let (csb_timestamp, offset_position) = {
                                let csb_timestamp = channels.csb_telemetry.borrow().clone().map(|t| t.timestamp);

                                if csb_timestamp.is_none() {
                                    warn!("no csb telemetry data available for image capture")
                                }

                                let offset_position = if let (Some(pixhawk_telemetry), Some(csb_timestamp)) = (&pixhawk_telemetry, &csb_timestamp) {
                                    // velocity in meters per second east and north
                                    let (vx, vy, vz) = pixhawk_telemetry.velocity;
                                    let delay = *csb_timestamp - pixhawk_telemetry.timestamp;
                                    let delay_seconds = delay.num_milliseconds() as f32 / 1000.;

                                    // angle we are traveling at
                                    let heading = pixhawk_telemetry.plane_attitude.yaw;

                                    // distance traveled since we received gps from pixhawk
                                    let distance_xy = f32::sqrt(vx * vx + vy * vy) * delay_seconds;
                                    let offset_coords = pixhawk_telemetry.position.point.haversine_destination(heading, distance_xy);
                                    let offset_altitude_rel = pixhawk_telemetry.position.altitude_rel + vz * delay_seconds;
                                    let offset_altitude_msl = pixhawk_telemetry.position.altitude_msl + vz * delay_seconds;

                                    Some(Point3D {
                                        point: offset_coords,
                                        altitude_msl: offset_altitude_msl,
                                        altitude_rel: offset_altitude_rel,
                                    })
                                } else {
                                    None
                                };

                                (csb_timestamp, offset_position)
                            };

                            #[cfg(not(feature = "csb"))]
                            let (csb_timestamp, offset_position) = (None, None);

                            let image_filename = match save(&image_save_dir, &image_name, &image_data, &pixhawk_telemetry, offset_position, csb_timestamp, cc_timestamp).await {
                                Ok(image_filename) => image_filename,
                                Err(err) => {
                                  warn!("failed to download image: {}", err);
                                  continue;
                                }
                            };

                            let _ = channels.image_event.send(ImageClientEvent {
                              data: image_data,
                              file: image_filename,
                              telemetry: pixhawk_telemetry
                            });
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

async fn save(
    image_save_dir: impl AsRef<Path>,
    name: &str,
    image: &Vec<u8>,
    pixhawk_telemetry: &Option<Telemetry>,
    offset_position: Option<Point3D>,
    csb_telemetry: Option<chrono::DateTime<chrono::Local>>,
    cc_timestamp: Option<chrono::DateTime<chrono::Local>>,
) -> anyhow::Result<PathBuf> {
    let mut image_path = image_save_dir.as_ref().to_owned();
    image_path.push(&name);
    debug!("writing image to file '{}'", image_path.to_string_lossy());

    let mut image_file = File::create(&image_path)
        .await
        .context("failed to create image file")?;

    image_file
        .write_all(&image[..])
        .await
        .context("failed to save image")?;

    info!("wrote image to file '{}'", image_path.to_string_lossy());

    let telem_path = image_path.with_extension("json");

    debug!(
        "writing telemetry to file '{}'",
        telem_path.to_string_lossy()
    );

    let cc_timestamp =
        cc_timestamp.map(|cc_timestamp| cc_timestamp.format(ISO_8601_FORMAT).to_string());

    let telem_bytes = serde_json::to_vec(&serde_json::json!({
        "pixhawk_telemetry": pixhawk_telemetry,
        "offset_position": offset_position,
        "csb_timestamp": csb_telemetry,
        "cc_timestamp": cc_timestamp,
    }))
    .context("failed to serialize telemetry to JSON")?;

    let mut telem_file = File::create(telem_path)
        .await
        .context("failed to create telemetry file")?;

    telem_file
        .write_all(&telem_bytes[..])
        .await
        .context("failed to write telemetry data to file")?;

    Ok(image_path)
}
