use crate::task::control::ControlTask;
use ps_main_camera::CameraRequest;
use ps_main_camera::MainCameraConfig;
use ps_telemetry::Telemetry;
use tokio::sync::watch;
pub fn create_tasks(
    config: MainCameraConfig,
    camera_ctrl_cmd_tx: flume::Sender<CameraRequest>,
    telem_rx: watch::Receiver<Telemetry>,
) -> anyhow::Result<(ControlTask)> {
    let control_task = ControlTask::new(camera_ctrl_cmd_tx, telem_rx);

    Ok(control_task)
}
mod control;
mod search;
mod standby;
mod testing;
mod util;
