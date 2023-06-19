use crate::config::ModesConfig;
use crate::task::control::ModesTask;
use ps_gimbal::GimbalRequest;
use ps_gimbal::GimbalResponse;
use ps_main_camera::CameraRequest;
use ps_main_camera::CameraResponse;
use ps_telemetry::Telemetry;
use tokio::sync::watch;

pub fn create_tasks(
    modes_config: ModesConfig,
    camera_ctrl_cmd_tx: flume::Sender<(
        CameraRequest,
        tokio::sync::oneshot::Sender<Result<CameraResponse, anyhow::Error>>,
    )>,
    telem_rx: watch::Receiver<Telemetry>,
    gimbal_tx: Option<
        flume::Sender<(
            GimbalRequest,
            tokio::sync::oneshot::Sender<Result<GimbalResponse, anyhow::Error>>,
        )>,
    >,
) -> anyhow::Result<ModesTask> {
    let control_task = ModesTask::new(modes_config, camera_ctrl_cmd_tx, telem_rx, gimbal_tx);

    Ok(control_task)
}

mod control;
mod search;
mod server;
mod standby;
mod testing;
mod util;

pub use server::serve;
