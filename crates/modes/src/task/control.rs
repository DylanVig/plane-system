//file to control processing commands and then calling plane system modes and interacting between them
use crate::command::{ModeRequest, ModeResponse, SearchRequest};
use async_trait::async_trait;
use crossterm::event::{read, Event, KeyCode, KeyEvent, KeyModifiers};
use ps_client::ChannelCommandSink;
use ps_client::ChannelCommandSource;
use ps_client::Task;
use ps_main_camera::CameraRequest;
//use ps_telemetry::PixhawkTelemetry;
use super::util::{end_cc, start_cc, transition_by_distance};
use ps_telemetry::Telemetry;
use tokio::select;
use tokio::sync::watch;
use tokio::time;
use tokio::time::sleep;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

pub enum Modes {
    Search,
    Standby,
    None,
}

pub struct ControlTask {
    cmd_rx: ChannelCommandSource<ModeRequest, ModeResponse>,
    cmd_tx: ChannelCommandSink<ModeRequest, ModeResponse>,
    camera_ctrl_cmd_tx: flume::Sender<CameraRequest>,
    telem_rx: watch::Receiver<Telemetry>,
}

impl ControlTask {
    pub(super) fn new(
        camera_ctrl_cmd_tx: flume::Sender<CameraRequest>,
        telem_rx: watch::Receiver<Telemetry>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = flume::bounded::<ModeRequest, ModeResponse>(256);

        Self {
            camera_ctrl_cmd_tx,
            telem_rx,
            cmd_rx,
            cmd_tx,
        }
    }

    pub fn cmd(&self) -> ChannelCommandSink<ModeRequest, ModeResponse> {
        self.cmd_tx.clone()
    }
}

async fn time_search(active: u64, inactive: u64, main_camera_tx: flume::Sender<CameraRequest>) {
    let inactive_dur = Duration::new(inactive, 0);
    let active_dur = Duration::new(active, 0);
    loop {
        //assumes cc is not running on entry
        sleep(inactive_dur);
        start_cc(&main_camera_tx);
        sleep(active_dur);
        end_cc(&main_camera_tx);
    }
}

async fn distance_search(
    distance_threshold: u64,
    waypoint: Vec<geo::Point>,
    telemetry_rx: watch::Receiver<Telemetry>,
    main_camera_tx: flume::Sender<CameraRequest>,
) {
    let mut enter = true; // start assuming not in range
    loop {
        transition_by_distance(&waypoint, &telemetry_rx, distance_threshold, enter);
        start_cc(&main_camera_tx);
        //checking for exit to end cc
        enter = false;
        transition_by_distance(&waypoint, &telemetry_rx, distance_threshold, enter);
        end_cc(&main_camera_tx);
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
            let ctrl_evt_tx = self.cmd_tx;
            loop {
                match self.cmd_rx.recv_async().await {
                    Ok((req, ret)) => {
                        let result = match req {
                            ModeRequest::Inactive => todo!(),
                            ModeRequest::LivestreamOnly => todo!(),
                            ModeRequest::ZoomControl(req) => todo!(),
                            ModeRequest::Search(req) => match req {
                                SearchRequest::Time { active, inactive } => {
                                    time_search(active, inactive, self.camera_ctrl_cmd_tx);
                                    Ok(())
                                }
                                SearchRequest::Distance { distance, waypoint } => {
                                    distance_search(
                                        distance,
                                        waypoint,
                                        self.telem_rx,
                                        self.camera_ctrl_cmd_tx,
                                    );
                                    Ok(())
                                }
                                SearchRequest::Manual { start } if start => {
                                    start_cc(&self.camera_ctrl_cmd_tx);
                                    Ok(())
                                }
                                SearchRequest::Manual { start } => {
                                    end_cc(&self.camera_ctrl_cmd_tx);
                                    Ok(())
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
