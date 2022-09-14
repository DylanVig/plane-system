mod control;
mod download;
mod event;
mod util;

use std::sync::Arc;

use anyhow::Context;
pub use control::*;
pub use download::*;
pub use event::*;
use ps_client::ChannelCommandSink;
use tokio::sync::RwLock;

use crate::{interface, CameraRequest, CameraResponse, MainCameraConfig};

pub fn create_tasks(
    config: MainCameraConfig,
) -> anyhow::Result<(
    ControlTask,
    EventTask,
    DownloadTask,
    ChannelCommandSink<CameraRequest, CameraResponse>,
    flume::Receiver<Download>,
)> {
    let mut interface =
        interface::CameraInterface::new().context("failed to initialize usb camera interface")?;
    interface.connect().context("failed to set up camera")?;

    let interface = Arc::new(RwLock::new(interface));

    let (cmd_tx, cmd_rx) = flume::bounded(256);
    let (evt_tx, evt_rx) = flume::bounded(256);
    let (download_tx, download_rx) = flume::bounded(256);

    let control_task = ControlTask {
        interface: interface.clone(),
        cmd_rx,
    };

    let event_task = EventTask {
        interface: interface.clone(),
        evt_tx,
    };

    let download_task = DownloadTask {
        interface,
        evt_rx: evt_rx.clone(),
        download_tx,
    };

    Ok((control_task, event_task, download_task, cmd_tx, download_rx))
}
