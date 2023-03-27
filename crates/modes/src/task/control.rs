//file to control processing commands and then calling plane system modes and interacting between them

use crossterm::event::{read, Event, KeyCode, KeyEvent, KeyModifiers};
use ps_main_camera::CameraRequest;

use tokio::time;

pub enum Modes {
    Search,
    Standby,
    None,
}

pub struct ControlTask {
    cmd_rx: ChannelCommandSource<SearchRequest, SearchResponse>,
    cmd_tx: ChannelCommandSink<SearchRequest, SearchResponse>,
    camera_ctrl_cmd_tx: flume::Sender<CameraRequest>,
    telem_rx: watch::Receiver<Telemetry>,
}

impl ControlTask {
    pub(super) fn new(
        camera_ctrl_cmd_tx: flume::Sender<CameraRequest>,
        telem_rx: watch::Receiver<Telemetry>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = flume::bounded(256);

        Self {
            camera_cntrl_cmd_tx,
            telem_rx,
            cmd_rx,
            cmd_tx,
        }
    }

    pub fn cmd(&self) -> ChannelCommandSink<SearchRequest, SearchResponse> {
        self.cmd_tx.clone()
    }
}

async fn time_search(active: u16, inactive: u16, main_camera_tx: flume::Sender<CameraRequest>) {
    loop {
        //assumes cc is not running on entry
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
    main_camera_tx: flume::Sender<CameraRequest>,
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
                                //Use .select with within distance to cancel search when out of range
                                SearchRequest::Time(active, inactive) =>
                                //standby mode

                                //wait for sometime

                                //searchmode

                                //figure out how to call cc

                                //time

                                //ad nauseaum
                                {
                                    todo!()
                                }
                                SearchRequest::Distance(distance, waypoint) => todo!(),
                                SearchRequest::Manual() =>
                                //wait for buttonpress

                                //run until the buttonpress works
                                {
                                    todo!()
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
