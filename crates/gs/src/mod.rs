use std::{ffi::OsStr, str::FromStr, sync::Arc};

use anyhow::Context;
use clap::{AppSettings, Subcommand};
use futures::{select, FutureExt};

use reqwest;

use crate::state::*;
use serde_json::json;

use crate::{image::ImageClientEvent, Channels};

#[derive(Subcommand, Debug, Clone)]
#[clap(setting(AppSettings::NoBinaryName))]
#[clap(rename_all = "kebab-case")]
pub enum GroundServerRequest {}

pub struct GroundServerClient {
    channels: Arc<Channels>,
    http: reqwest::Client,
    base_url: reqwest::Url,
}

impl GroundServerClient {
    pub fn new(channels: Arc<Channels>, base_url: String) -> anyhow::Result<Self> {
        Ok(GroundServerClient {
            channels,
            base_url: reqwest::Url::from_str(&base_url).context("invalid ground server url")?,
            http: reqwest::Client::new(),
        })
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let mut interrupt_recv = self.channels.interrupt.subscribe();
        let mut image_recv = self.channels.image_event.subscribe();

        let interrupt_fut = interrupt_recv.recv().fuse();

        futures::pin_mut!(interrupt_fut);

        loop {
            select! {
                image_evt = image_recv.recv().fuse() => {
                    if let Ok(ImageClientEvent {
                        file,
                        data,
                        telemetry,
                    }) = image_evt
                    {
                        debug!("image download detected, uploading file to ground server");

                        let file_name = file
                            .file_name()
                            .map(OsStr::to_string_lossy)
                            .expect("image has no filename");

                        let telemetry_info = self.channels.pixhawk_telemetry.borrow().clone();

                        if telemetry_info.is_none() {
                            warn!("no telemetry data available for image capture")
                        }

                        self.send_image(data.as_ref(), file_name.to_string(), telemetry).await?;
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
        file_name: String,
        telemetry: Option<Telemetry>,
    ) -> anyhow::Result<()> {
        let file_name = file_name.to_lowercase();

        let mime_type = {
            let file_ext = file_name.split(".").last();

            match file_ext {
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("mp4") => "video/mp4",
                ext => {
                    bail!(
                        "unknown mime type for image file received from camera with extension {:?}",
                        ext
                    );
                }
            }
        };

        let endpoint = self
            .base_url
            .join("/api/v1/image")
            .expect("could not create image upload url");

        let timestamp = chrono::Utc::now().timestamp_millis();

        let json = if let Some(telemetry) = telemetry {
            json!({
                "timestamp": timestamp,
                "imgMode": "fixed",
                "fov": 60.0,
                "telemetry": {
                    "altitude": telemetry.position.altitude_rel,
                    "planeYaw": telemetry.plane_attitude.yaw,
                    "gps": {
                        "longitude": telemetry.position.point.x(),
                        "latitude": telemetry.position.point.y(),
                    },
                    "gimOrt": {
                        "pitch": telemetry.gimbal_attitude.pitch,
                        "roll": telemetry.gimbal_attitude.roll,
                    }
                }
            })
        } else {
            if cfg!(debug_assertions) {
                warn!("no telemetry information available, uploading filler telemetry info");

                json!({
                    "timestamp": timestamp,
                    "imgMode": "fixed",
                    "fov": 60.0,
                    "telemetry": {
                        "altitude": 0.0,
                        "planeYaw": 0.0,
                        "gps": {
                            "latitude": 0.0,
                            "longitude": 0.0,
                        },
                        "gimOrt": {
                            "pitch": 0.0,
                            "roll": 0.0,
                        }
                    }
                })
            } else {
                bail!("no telemetry information available, cannot upload to ground server");
            }
        };

        let form = reqwest::multipart::Form::new()
            .part("json", reqwest::multipart::Part::text(json.to_string()))
            .part(
                "files",
                reqwest::multipart::Part::bytes(Vec::from(data))
                    .file_name(file_name)
                    .mime_str(mime_type)?,
            );

        let res = self.http.post(endpoint).multipart(form).send().await?;

        res.error_for_status()
            .context("uploading image and telemetry to ground server failed")?;

        debug!("uploaded image and telemetry to ground server");

        Ok(())
    }
}
