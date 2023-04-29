//file to control processing commands and then calling plane system modes and interacting between them
use crate::command::{ModeRequest, ModeResponse, SearchRequest};
use crate::task::control::SearchModeError::CameraRequestError;
use crate::task::control::SearchModeError::WaypointError;
use async_trait::async_trait;
use ps_client::ChannelCommandSink;
use ps_client::ChannelCommandSource;
use ps_client::Task;
use ps_main_camera::CameraRequest;
use ps_main_camera::CameraResponse;
//use ps_telemetry::PixhawkTelemetry;
use super::util::{end_cc, start_cc, transition_by_distance};
use anyhow::Error;
use ps_telemetry::Telemetry;
use thiserror::Error;
use tokio::select;
use tokio::sync::watch;
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
    camera_ctrl_cmd_tx: flume::Sender<(
        CameraRequest,
        tokio::sync::oneshot::Sender<Result<CameraResponse, anyhow::Error>>,
    )>,
    telem_rx: watch::Receiver<Telemetry>,
}

impl ControlTask {
    pub(super) fn new(
        camera_ctrl_cmd_tx: flume::Sender<(
            CameraRequest,
            tokio::sync::oneshot::Sender<Result<CameraResponse, anyhow::Error>>,
        )>,
        telem_rx: watch::Receiver<Telemetry>,
    ) -> Self {
        let (cmd_tx, cmd_rx) = flume::bounded(256);

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

async fn time_search(
    active: u64,
    inactive: u64,
    main_camera_tx: flume::Sender<(
        CameraRequest,
        tokio::sync::oneshot::Sender<Result<CameraResponse, anyhow::Error>>,
    )>,
) -> Result<(), SearchModeError> {
    let inactive_dur = Duration::new(inactive, 0);
    let active_dur = Duration::new(active, 0);
    loop {
        //assumes cc is not running on entry
        sleep(inactive_dur);
        match start_cc(main_camera_tx.clone()).await {
            Ok(_) => {}
            Err(e) => return Err(CameraRequestError),
        }
        sleep(active_dur);
        match end_cc(main_camera_tx.clone()).await {
            Ok(_) => {}
            Err(e) => return Err(CameraRequestError),
        }
    }
}

#[derive(Error, Debug)]
pub enum SearchModeError {
    #[error("could not send request to the camera")]
    CameraRequestError,
    #[error("invalid waypoint entered")]
    WaypointError,
}

async fn distance_search(
    distance_threshold: u64,
    waypoint: Vec<geo::Point>,
    telemetry_rx: watch::Receiver<Telemetry>,
    main_camera_tx: flume::Sender<(
        CameraRequest,
        tokio::sync::oneshot::Sender<Result<CameraResponse, anyhow::Error>>,
    )>,
) -> Result<(), SearchModeError> {
    let mut enter = true; // start assuming not in range
    loop {
        match transition_by_distance(&waypoint, telemetry_rx.clone(), distance_threshold, enter)
            .await
        {
            Ok(_) => {}
            Err(e) => return Err(WaypointError),
        }
        match start_cc(main_camera_tx.clone()).await {
            Ok(_) => {}
            Err(e) => return Err(CameraRequestError),
        }
        //checking for exit to end cc
        enter = false;
        match transition_by_distance(&waypoint, telemetry_rx.clone(), distance_threshold, enter)
            .await
        {
            Ok(_) => {}
            Err(e) => return Err(WaypointError),
        }
        match end_cc(main_camera_tx.clone()).await {
            Ok(_) => {}
            Err(e) => return Err(CameraRequestError),
        }
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
            //let camera_cntrl = self.camera_ctrl_cmd_tx;

            loop {
                match self.cmd_rx.recv_async().await {
                    Ok((req, ret)) => {
                        let result: Result<ModeResponse, Error> = match req {
                            ModeRequest::Inactive => todo!(),
                            ModeRequest::LivestreamOnly => todo!(),
                            ModeRequest::ZoomControl(req) => todo!(),
                            ModeRequest::Search(req) => match req {
                                SearchRequest::Time { active, inactive } => {
                                    time_search(active, inactive, self.camera_ctrl_cmd_tx.clone());
                                    Ok(ModeResponse::Response)
                                }
                                SearchRequest::Distance { distance, waypoint } => {
                                    distance_search(
                                        distance,
                                        waypoint,
                                        self.telem_rx.clone(),
                                        self.camera_ctrl_cmd_tx.clone(),
                                    );
                                    Ok(ModeResponse::Response)
                                }
                                SearchRequest::Manual { start } if start => {
                                    start_cc(self.camera_ctrl_cmd_tx.clone());
                                    Ok(ModeResponse::Response)
                                }
                                SearchRequest::Manual { start } => {
                                    end_cc(self.camera_ctrl_cmd_tx.clone());
                                    Ok(ModeResponse::Response)
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
