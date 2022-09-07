pub use ps_main_camera::*;

use crate::Channels;

pub async fn run(
  channels: Arc<Channels>,
  command_rx: flume::Receiver<CameraCommand>,
) -> anyhow::Result<()> {
  let mut interface = CameraInterface::new().context("failed to create camera interface")?;

  let mut tries = 0;

  loop {
      match interface.connect().context("failed to connect to camera") {
          Ok(_) => break,
          Err(err) => {
              if tries > 3 {
                  return Err(err);
              }

              tries += 1;

              warn!("failed to connect to camera: {:?}", err);
              info!("retrying camera connection");
              if let Err(err) = interface.disconnect() {
                  warn!("failed to disconnect from camera: {:?}", err);
              }
          }
      }
  }

  trace!("initializing camera");

  let time_str = chrono::Local::now()
      .format("%Y%m%dT%H%M%S%.3f%:z")
      .to_string();

  trace!("setting time on camera to '{}'", &time_str);

  if let Err(err) = interface.set(CameraPropertyCode::DateTime, ptp::PtpData::STR(time_str)) {
      warn!("could not set date/time on camera: {:?}", err);
  }

  let state = interface.update().context("could not get camera state")?;

  let state = state
      .into_iter()
      .filter_map(|p| {
          if let Some(property_code) =
              <CameraPropertyCode as FromPrimitive>::from_u16(p.property_code)
          {
              Some((property_code, p))
          } else {
              None
          }
      })
      .collect();

  let (ptp_tx, _) = broadcast::channel(256);

  info!("initialized camera");

  let interface = Arc::new(interface);

  let (interface_tx, interface_rx) = flume::unbounded();

  let semaphore = Arc::new(Semaphore::new(1));

  let interface_req_buf = CameraInterfaceRequestBuffer {
      chan: interface_tx,
      semaphore: semaphore.clone(),
  };

  let mut futures = Vec::new();
  let mut task_names = Vec::new();

  let download_task = spawn_with_name(
      "camera download",
      run_download(
          interface_req_buf.clone(),
          ptp_tx.subscribe(),
          channels.camera_event.clone(),
      ),
  );

  task_names.push("download");
  futures.push(download_task);

  let cmd_task = spawn_with_name(
      "camera cmd",
      run_commands(
          interface_req_buf.clone(),
          ptp_tx.subscribe(),
          command_rx,
          channels.camera_event.clone(),
      ),
  );

  task_names.push("cmd");
  futures.push(cmd_task);

  let interface_task = spawn_blocking_with_name("camera interface", {
      let interface = interface.clone();
      let interrupt_rx = channels.interrupt.subscribe();
      move || run_interface(interface, state, interface_rx, interrupt_rx)
  });

  task_names.push("interface");
  futures.push(interface_task);

  let event_task = spawn_blocking_with_name("camera events", {
      let interface = interface.clone();
      let interrupt_rx = channels.interrupt.subscribe();
      move || run_events(interface, semaphore, ptp_tx, interrupt_rx)
  });

  task_names.push("event");
  futures.push(event_task);

  Ok(())
}
