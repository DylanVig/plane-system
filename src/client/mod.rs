use anyhow::Context;
use chrono::DateTime;
use reqwest;

use crate::state::TelemetryInfo;
use serde::Serialize;
use serde_json::json;


pub async fn upload_image(
    server: reqwest::Url,
    data: Vec<u8>,
    mime_type: &str,
    telemetry: TelemetryInfo,
) -> anyhow::Result<()> {
    let endpoint = server
        .join("/api/v1/image")
        .expect("could not create image upload url");

    let client = reqwest::Client::new();

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

    client.post(endpoint).multipart(form).send().await?;

    Ok(())
}
