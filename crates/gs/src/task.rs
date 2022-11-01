use crate::GsConfig;
use anyhow::bail;
use anyhow::Context;
use flume;

use log::{debug, trace, warn};

use ps_telemetry::Telemetry;
use reqwest;
use serde_json::json;
use std::path::PathBuf;
use std::{str::FromStr, sync::Arc};
use tokio::select;
use tokio_util::sync::CancellationToken;

///This is a struct to contain the data to send to the ground server
pub enum GsCommand {
    UploadImage {
        data: Arc<Vec<u8>>,
        file: PathBuf,
        telemetry: Option<Telemetry>,
    },
}

///Creates task returning an upload task and transmitting channel
pub fn create_task(config: GsConfig) -> anyhow::Result<UploadTask> {
    let (cmd_tx, cmd_rx) = flume::bounded(256);

    Ok(UploadTask {
        base_url: reqwest::Url::from_str(&config.address).context("invalid ground server url")?,
        http_client: reqwest::Client::new(),
        cmd_rx,
        cmd_tx,
    })
}

///Task has the client communicate to the ground server. cmd_rx communicates with the camera crate.
///Listens for command and uploads file to ground server.
pub struct UploadTask {
    base_url: reqwest::Url,
    http_client: reqwest::Client,
    //receiving half of the channel
    cmd_rx: flume::Receiver<GsCommand>,
    //transmitting half of the channel
    cmd_tx: flume::Sender<GsCommand>,
}

///Sends image to the ground server
impl UploadTask {
    //this waits for a command, then send image to ground server
    pub async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        //extract the input parameters of ground server client and channnel to recieve commands
        let Self {
            cmd_rx,
            cmd_tx: _,
            base_url,
            http_client,
        } = *self;

        //wait for image to be send
        let cmd_loop = async {
            trace!("Beginning image send process");

            while let Ok(cmd) = cmd_rx.recv_async().await {
                //once image is recieved match data,file, and telemtry to now send to ground server
                let _result = match cmd {
                    GsCommand::UploadImage {
                        data,
                        file,
                        telemetry,
                    } => {
                        //image download completed, starting upload process
                        debug!("image download detected, uploading file to ground server");

                        if telemetry.is_none() {
                            warn!("no telemetry data available for image capture")
                        }

                        send_image(
                            data.as_ref(),
                            file.file_name()
                                .expect("invalid image file name")
                                .to_string_lossy()
                                .into_owned(),
                            telemetry,
                            &http_client,
                            &base_url,
                        )
                        .await?;
                    }
                };
            }

            Ok::<_, anyhow::Error>(())
        };

        select! {
            _ = cancel.cancelled() => {},
            res = cmd_loop => { res? },
        };

        Ok(())
    }
}

// Sends an image to the ground server.
async fn send_image(
    file_data: &[u8],
    file_name: String,
    telemetry: Option<Telemetry>,
    http: &reqwest::Client,
    base_url: &reqwest::Url,
) -> anyhow::Result<()> {
    let file_name = file_name.to_lowercase();

    let mime_type = {
        let file_ext = file_name.split(".").last();
        //tells the reciever the type of data being recieved
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

    let endpoint = base_url
        .join("/api/v1/image")
        .expect("could not create image upload url");

    let timestamp = chrono::Utc::now().timestamp_millis();

    let json = if let Some(telemetry) = telemetry {
        json!({
            "timestamp": timestamp,
            "imgMode": "fixed",
            "fov": 60.0,
            "telemetry": {
                "altitude": telemetry.pixhawk.as_ref().map(|p| p.position.0.altitude_msl),
                "planeYaw": telemetry.pixhawk.as_ref().map(|p| p.attitude.0.yaw),
                "gps": {
                    "longitude": telemetry.pixhawk.as_ref().map(|p| p.position.0.point.x()),
                    "latitude": telemetry.pixhawk.as_ref().map(|p| p.position.0.point.y()),
                },
                "gimOrt": {
                    "pitch": telemetry.pixhawk.as_ref().map(|p| p.attitude.0.pitch),
                    "roll": telemetry.pixhawk.as_ref().map(|p| p.attitude.0.roll),
                }
            }
        })
    //when there's no telemetry (no pixhawk, or not connected, etc), sends the telemetry below
    } else {
        //runs when in debug mode
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
            //in release mode, will not upload anything if there is no telemetry
            bail!("no telemetry information available, cannot upload to ground server");
        }
    };

    let form = reqwest::multipart::Form::new()
        .part("json", reqwest::multipart::Part::text(json.to_string()))
        .part(
            "files",
            reqwest::multipart::Part::bytes(Vec::from(file_data))
                .file_name(file_name)
                .mime_str(mime_type)?,
        );

    let res = http.post(endpoint).multipart(form).send().await?;

    res.error_for_status()
        .context("uploading image and telemetry to ground server failed")?;

    debug!("uploaded image and telemetry to ground server");

    Ok(())
}
