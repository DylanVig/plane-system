//file to control processing commands and then calling plane system modes and itneracting between them

use ps_main_camera::CameraRequest;

pub enum Modes {
    Search,
    Standby,
    None,
}

pub struct ControlTask {
    ctrl_evt_rx: flume::Receiver<ControlEvent>,
    ctrl_evt_tx: flume::Sender<ControlEvent>,
    cmd_rx: ChannelCommandSource<CameraRequest, CameraResponse>,
    cmd_tx: ChannelCommandSink<CameraRequest, CameraResponse>,
}

impl ControlTask {
    pub(super) fn new() -> Self {
        let (cmd_tx, cmd_rx) = flume::bounded(256);
        let (ctrl_evt_tx, ctrl_evt_rx) = flume::bounded(256);

        Self {
            ctrl_evt_rx,
            ctrl_evt_tx,
            cmd_rx,
            cmd_tx,
        }
    }

    pub fn cmd(&self) -> ChannelCommandSink<CameraRequest, CameraResponse> {
        self.cmd_tx.clone()
    }

    pub fn event(&self) -> flume::Receiver<ControlEvent> {
        self.ctrl_evt_rx.clone()
    }
}

async fn time_search(active: u16, inactive: u16, main_camera_tx: flume::Sender<CameraRequest>) {
    loop { //assumes cc is not running on entry
        sleep(inactive); 
        start_cc(main_camera_tx); 
        sleep(active); 
        end_cc(main_camera_tx);
    }
  
}

async fn distance_search(
    distance_threshold: u64,
    waypoint: Vec<geo::Point>,
    telemetry_rx: watch::Receiver<PixhawkTelemetry>,
    main_camera_tx: flume::Sender<CameraRequest>
) {
    let enter = true; // start assuming not in range
    loop {
        transition_by_distance(distance_threshold, waypoint, telemetry_rx, enter);
        start_cc(main_camera_tx);
        //checking for exit to end cc
        enter = false;
        transition_by_distance(distance_threshold, waypoint, telemetry_rx, enter);
        end_cc(main_camera_tx);
        enter = true;
    }

}

#[async_trait]
impl Task for ControlTask {
    fn name(&self) -> &'static str {
        "modes/control"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let loop_fut = async move {
            let ctrl_evt_tx = self.ctrl_evt_tx;
            loop {
                match self.cmd_rx.recv_async().await {
                    Ok((req, ret)) => {
                        let result = match req {
                            ModeRequest::Inactive(_) => todo!(),
                            ModeRequest::ZoomControl(req) => todo!(),
                            ModeRequest::Search(req) => match req {
                                SearchRequest::Time(active, inactive) => {
                                    time_search(active, inactive, main_camera_tx);
                                }
                                SearchRequest::Distance(distance, waypoint) => {
                                    distance_search(distance, waypoint, telemetry_rx, main_camera_tx);
                                }
                                SearchRequest::Manual(start) if start => {
                                    start_cc(main_camera_tx);
                                }
                                SearchRequest::Manual(start) => {
                                    end_cc(main_camera_tx);
                                }
                               
                            },
                        };

                        let _ = ret.send(result);
                    }
                    Err(_) => break,
                }
            }

            Ok::<_, anyhow::Error>(())
        };

        select! {
          _ = cancel.cancelled() => {}
          res = loop_fut => { res? }
        }

        Ok(())
    }
}
