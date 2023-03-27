pub use ps_modes::control;

pub fn create_tasks(
    config: MainCameraConfig,
    camera_ctrl_cmd_tx: flume::Sender<CameraRequest>,
    telem_rx: watch::Receiver<Telemetry>,
) -> anyhow::Result<(ControlTask)> {
    let control_task = ControlTask::new(camera_cntrl_cmd_tx, telem_rx);

    Ok(control_task)
}
mod control;
mod search;
mod standby;
mod testing;
mod util;
