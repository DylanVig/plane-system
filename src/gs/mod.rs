use std::{ffi::OsStr, path::Path, sync::Arc};

use clap::AppSettings;
use futures::{select, FutureExt};
///! Functions for interfacing with the ground server.
use structopt::StructOpt;

use reqwest;

use crate::state::*;
use serde_json::json;

use crate::{CameraEvent, Channels};

#[derive(StructOpt, Debug, Clone)]
#[structopt(setting(AppSettings::NoBinaryName))]
#[structopt(rename_all = "kebab-case")]
pub enum GroundServerRequest {}

pub struct GroundServerClient {
    channels: Arc<Channels>,
    http: reqwest::Client,
    base_url: reqwest::Url,
}

impl GroundServerClient {
    pub fn new(channels: Arc<Channels>, base_url: reqwest::Url) -> anyhow::Result<Self> {
        Ok(GroundServerClient {
            channels,
            base_url,
            http: reqwest::Client::new(),
        })
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

                                let image_mime = if let Some(image_ext) = Path::new(&image_name).extension().and_then(OsStr::to_str) {
                                    let image_ext = image_ext.to_lowercase();

                                    match image_ext.as_str() {
                                        "jpg" | "jpeg" => "image/jpeg",
                                        "mp4" => "video/mp4",
                                        ext => {
                                            error!("unknown mime type for image file received from camera with extension {:?}", ext);
                                            continue;
                                        }
                                    }
                                } else {
                                    error!("unknown mime type for image file received from camera");
                                    continue;
                                };

                                let telemetry_info = self.channels.telemetry.borrow().clone().unwrap_or_else(|| {
                                    warn!("no telemetry data available for image capture");

                                    TelemetryInfo {
                                        plane_attitude: Attitude {
                                            roll: 30.0,
                                            pitch: 10.0,
                                            yaw: -20.0,
                                        },
                                        gimbal_attitude: Attitude {
                                            roll: 60.0,
                                            pitch: 70.0,
                                            yaw: -90.0,
                                        },
                                        position: Coords3D {
                                            latitude: -10.0,
                                            longitude: 30.0,
                                            altitude: 400.0,
                                        },
                                    }
                                });

                                self.send_image(
                                    image_data.as_ref(),
                                    image_name,
                                    image_mime,
                                    telemetry_info,
                                ).await?;
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

    /// Sends an image to the ground server.
    pub async fn send_image(
        &self,
        data: &[u8],
        image_name: String,
        mime_type: &str,
        telemetry: TelemetryInfo,
    ) -> anyhow::Result<()> {
        let endpoint = self
            .base_url
            .join("/api/v1/image")
            .expect("could not create image upload url");

        let timestamp = chrono::Utc::now().timestamp_millis();

        let json = json!({
            "timestamp": timestamp,
            "imgMode": "fixed",
            "fov": 60.0,
            "telemetry": {
                "altitude": telemetry.position.altitude,
                "planeYaw": telemetry.plane_attitude.yaw,
                "gps": {
                    "latitude": telemetry.position.latitude,
                    "longitude": telemetry.position.longitude,
                },
                "gimOrt": {
                    "pitch": telemetry.gimbal_attitude.pitch,
                    "roll": telemetry.gimbal_attitude.roll,
                }
            }
        });

        let form = reqwest::multipart::Form::new()
            .part("json", reqwest::multipart::Part::text(json.to_string()))
            .part(
                "files",
                reqwest::multipart::Part::bytes(Vec::from(data))
                    .file_name(image_name)
                    .mime_str(mime_type)?,
            );

        self.http.post(endpoint).multipart(form).send().await?;

        debug!("uploaded image and telemetry to ground server");

        Ok(())
    }
}
