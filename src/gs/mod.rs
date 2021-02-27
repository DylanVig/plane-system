use std::sync::Arc;

use clap::AppSettings;
///! Functions for interfacing with the ground server.
use structopt::StructOpt;

use reqwest;

use crate::state::TelemetryInfo;
use serde_json::json;

use crate::{
    Channels,
};

#[derive(StructOpt, Debug, Clone)]
#[structopt(setting(AppSettings::NoBinaryName))]
#[structopt(rename_all = "kebab-case")]
pub enum GroundServerRequest {
}

pub struct GroundServerClient {
    channels: Arc<Channels>,
    http: reqwest::Client,
    base_url: reqwest::Url,
}

impl GroundServerClient {
    pub fn connect(channels: Arc<Channels>, base_url: reqwest::Url) -> anyhow::Result<Self> {
        Ok(GroundServerClient {
            channels,
            base_url,
            http: reqwest::Client::new(),
        })
    }

    pub async fn run() {

    }

    pub async fn upload_image(
        &self,
        data: Vec<u8>,
        mime_type: &str,
        telemetry: TelemetryInfo,
    ) -> anyhow::Result<()> {
        let endpoint = self
            .base_url
            .join("/api/v1/image")
            .expect("could not create image upload url");

        let json = json!({
            "timestamp": 0,
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
                reqwest::multipart::Part::bytes(data).mime_str(mime_type)?,
            );

        self.http.post(endpoint).multipart(form).send().await?;

        Ok(())
    }
}
