use crate::task::control::ControlTask;
use ps_main_camera::CameraRequest;
use ps_main_camera::CameraResponse;
use ps_telemetry::Telemetry;
use ps_gimbal::GimbalResponse;
use ps_gimbal::GimbalRequest;
use tokio::sync::watch;
use crate::config::ModesConfig;

pub fn create_tasks(
    modes_config: ModesConfig,
    camera_ctrl_cmd_tx: flume::Sender<(
        CameraRequest,
        tokio::sync::oneshot::Sender<Result<CameraResponse, anyhow::Error>>,
    )>,
    telem_rx: watch::Receiver<Telemetry>,
    gimbal_tx: flume::Sender<(GimbalRequest, tokio::sync::oneshot::Sender<Result<GimbalResponse, anyhow::Error>>)>,
) -> anyhow::Result<ControlTask> {
    let control_task = ControlTask::new(modes_config, camera_ctrl_cmd_tx, telem_rx, gimbal_tx);

    Ok(control_task)
}
mod control;
mod search;
mod standby;
mod testing;
mod util;
