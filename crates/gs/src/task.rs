use crate::GsConfig;
use anyhow::bail;
use anyhow::Context;
use flume;
use futures::FutureExt;
use log::debug;
use log::trace;
use log::warn;
use ps_client::{ChannelCommandSink, Task};
use ps_telemetry::Telemetry;
use reqwest;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::{ffi::OsStr, str::FromStr, sync::Arc};
use tokio::select;
use tokio_util::sync::CancellationToken;

//enum to send command to ground server
pub enum GsCommand {
    UploadImage {
        data: Arc<Vec<u8>>,
        file: PathBuf,
        telemetry: Option<Telemetry>,
    },
}

//create task from aux camera, save crate
//purpose of this is to establish connection to ground server, the establihs a task to listen for communication from camera crate
//return the communication channle it created
//I made the channel two way, but it needs to be one
pub fn create_task(config: GsConfig) -> anyhow::Result<UploadTask> {
    //here I want to establish connection to ground server

    //create channel
    let (cmd_tx, cmd_rx) = flume::bounded(256);

    //Here I want to establish a task
    Ok(UploadTask {
        base_url: reqwest::Url::from_str(&config.address).context("invalid ground server url")?,
        http: reqwest::Client::new(),
        cmd_rx,
    })
}

//Task has the client to ocmmunicate to ground server and cmd_rx to communicate with camera crate
pub struct UploadTask {
    base_url: reqwest::Url,
    http: reqwest::Client,
    //receiving half of the channel
    cmd_rx: flume::Receiver<GsCommand>,
}

impl UploadTask {
    //this should wait for a command, then send image to ground server
    pub async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        //extract the input parameters of ground server client and channnel to recieve commands
        let Self {
            cmd_rx,
            base_url,
            http,
        } = *self;

        //wait for image to be send
        let cmd_loop = async {
            trace!("Beginning image send process");

            while let Ok(cmd) = cmd_rx.recv_async().await {
                //once image is recieved match data,file, and telemtry to now send to ground server
                let result = match cmd {
                    GsCommand::UploadImage {
                        data,
                        file,
                        telemetry,
                    } => {
                        // start image download process
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
                            &http,
                            &base_url,
                        )
                        .await?;
                    }
                };
            }

            //if ever gets past loop, then its an error cause that loop supposed ot be infinite
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
pub async fn send_image(
    file_data: &[u8],
    file_name: String,
    telemetry: Option<Telemetry>,
    http: &reqwest::Client,
    base_url: &reqwest::Url,
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
                "altitude": telemetry.location.0.altitude_msl,
                "planeYaw": telemetry.orientation.0.yaw,
                "gps": {
                    "longitude": telemetry.location.0.point.x(),
                    "latitude": telemetry.location.0.point.y(),
                },
                "gimOrt": {
                    "pitch": telemetry.orientation.0.pitch,
                    "roll": telemetry.orientation.0.roll,
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
