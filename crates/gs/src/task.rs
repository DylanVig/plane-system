use crate::state::*;
use crate::{image::ImageClientEvent, Channels};
use anyhow::Context;
use async_trait::async_trait;
use clap::{AppSettings, Subcommand};
use flume;
use futures::{select, FutureExt};
use ps_client::Task;
use reqwest;
use serde_json::json;
use std::{ffi::OsStr, str::FromStr, sync::Arc};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use telemetry::Telemetry;
use tokio::select;
use tokio_util::sync::CancellationToken;

// use crate::{gs}

pub enum GsCommand {
    UploadImage {
        data: Arc<Vec<u8>>,
        telemetry: Option<Telemetry>,
    },
}

pub type GsCommand = flume::Sender<GsCommand>;

pub struct EventTask {
    interface: GsInterface,
    // cmd_rx:
}

impl EventTask {
    //pub async fn send_image()
}

//ground server enum -- send image
pub struct ImageClientEvent {
    data: Arc<Vec<u8>>,
    file: PathBuf,
    telemetry: Option<Telemetry>,
}

//flume::Sender

// pub type Command<
