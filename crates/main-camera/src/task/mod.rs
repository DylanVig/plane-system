mod control;
mod download;
mod event;
mod live;
mod util;

use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
};

use anyhow::Context;
pub use control::*;
pub use download::*;
pub use event::*;
use log::*;

use ps_telemetry::Telemetry;
use tokio::sync::{watch, RwLock};

use crate::{
    interface::{self, PropertyCode},
    MainCameraConfig,
};

use self::live::LiveTask;

pub fn create_tasks(
    config: MainCameraConfig,
    telem_rx: watch::Receiver<Telemetry>,
) -> anyhow::Result<(ControlTask, EventTask, DownloadTask, LiveTask)> {
    let interface = Arc::new(RwLock::new(InterfaceGuard::new()?));

    let event_task = EventTask::new(interface.clone());
    let control_task = ControlTask::new(interface.clone(), event_task.events());
    let download_task = DownloadTask::new(config.download, interface.clone(), telem_rx, event_task.events());
    let live_task = LiveTask::new(interface);

    Ok((control_task, event_task, download_task, live_task))
}

/// A structure that initializes the camera interface when it is created, and
/// de-initializes it when it is dropped.
struct InterfaceGuard(interface::CameraInterface);

impl InterfaceGuard {
    pub fn new() -> anyhow::Result<Self> {
        let mut interface = interface::CameraInterface::new()
            .context("failed to initialize usb camera interface")?;
        interface.connect().context("failed to set up camera")?;

        debug!("initializing camera");

        let time_str = chrono::Local::now()
            .format("%Y%m%dT%H%M%S%.3f%:z")
            .to_string();

        info!("setting time on camera to '{}'", &time_str);

        if let Err(err) = interface.set(PropertyCode::DateTime, ptp::Data::STR(time_str)) {
            warn!("could not set time on camera: {:?}", err);
        }

        Ok(Self(interface))
    }
}

impl Deref for InterfaceGuard {
    type Target = interface::CameraInterface;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for InterfaceGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Drop for InterfaceGuard {
    fn drop(&mut self) {
        if let Err(err) = self.0.disconnect() {
            error!("failed to disconnect safely from camera: {err:?}");
        }
    }
}
