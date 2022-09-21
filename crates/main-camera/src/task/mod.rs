mod control;
mod download;
mod event;
mod util;

use std::sync::Arc;

use anyhow::Context;
pub use control::*;
pub use download::*;
pub use event::*;
use log::*;

use tokio::sync::RwLock;

use crate::{
    interface::{self, PropertyCode},
    MainCameraConfig,
};

pub fn create_tasks(
    _config: MainCameraConfig,
) -> anyhow::Result<(ControlTask, EventTask, DownloadTask)> {
    let mut interface =
        interface::CameraInterface::new().context("failed to initialize usb camera interface")?;
    interface.connect().context("failed to set up camera")?;

    debug!("initializing camera");

    let time_str = chrono::Local::now()
        .format("%Y%m%dT%H%M%S%.3f%:z")
        .to_string();

    info!("setting time on camera to '{}'", &time_str);

    if let Err(err) = interface.set(PropertyCode::DateTime, ptp::PtpData::STR(time_str)) {
        warn!("could not set time on camera: {:?}", err);
    }

    let interface = Arc::new(RwLock::new(interface));

    let control_task = ControlTask::new(interface.clone());
    let event_task = EventTask::new(interface.clone());
    let download_task = DownloadTask::new(interface, event_task.events());

    Ok((control_task, event_task, download_task))
}
