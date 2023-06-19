//! file to control processing commands and then calling plane system modes and interacting between them
use crate::command::{ModeError, ModeRequest, ModeResponse, SearchRequest};
use crate::config::{GimbalPosition, ModesConfig};
use crate::task::control::ModeError::CameraRequestError;
use crate::task::control::ModeError::GimbalRequestError;
use crate::task::control::ModeError::WaypointError;
use async_trait::async_trait;
use ps_client::CommandReceiver;
use ps_client::CommandSender;
use ps_client::Task;
use ps_gimbal::GimbalRequest;
use ps_gimbal::GimbalResponse;
use ps_main_camera::CameraRequest;
use ps_main_camera::CameraResponse;

//use ps_telemetry::PixhawkTelemetry;
use super::util::{capture, end_cc, rotate_gimbal, start_cc, transition_by_distance};
use anyhow::Error;
use ps_telemetry::Telemetry;
use thiserror::Error;
use tokio::select;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::time::sleep;
use tokio::time::Duration;
use tokio_util::sync::CancellationToken;

pub enum Modes {
    Search,
    Standby,
    None,
}

pub struct ModesTask {
    cmd_rx: flume::Receiver<(
        ModeRequest,
        oneshot::Sender<Result<ModeResponse, ModeError>>,
    )>,
    cmd_tx: flume::Sender<(
        ModeRequest,
        oneshot::Sender<Result<ModeResponse, ModeError>>,
    )>,
    camera_ctrl_cmd_tx: flume::Sender<(
        CameraRequest,
        tokio::sync::oneshot::Sender<Result<CameraResponse, anyhow::Error>>,
    )>,
    telem_rx: watch::Receiver<Telemetry>,
    gimbal_tx: Option<
        flume::Sender<(
            GimbalRequest,
            tokio::sync::oneshot::Sender<Result<GimbalResponse, Error>>,
        )>,
    >,
    modes_config: ModesConfig,
}

impl ModesTask {
    pub(super) fn new(
        modes_config: ModesConfig,
        camera_ctrl_cmd_tx: flume::Sender<(
            CameraRequest,
            tokio::sync::oneshot::Sender<Result<CameraResponse, anyhow::Error>>,
        )>,
        telem_rx: watch::Receiver<Telemetry>,
        gimbal_tx: Option<
            flume::Sender<(
                GimbalRequest,
                tokio::sync::oneshot::Sender<Result<GimbalResponse, Error>>,
            )>,
        >,
    ) -> Self {
        let (cmd_tx, cmd_rx) = flume::bounded::<(
            ModeRequest,
            oneshot::Sender<Result<ModeResponse, ModeError>>,
        )>(256);

        Self {
            camera_ctrl_cmd_tx,
            telem_rx,
            cmd_rx,
            cmd_tx,
            gimbal_tx,
            modes_config,
        }
    }

    pub fn cmd(
        &self,
    ) -> (flume::Sender<(
        ModeRequest,
        oneshot::Sender<Result<ModeResponse, ModeError>>,
    )>) {
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
) -> Result<(), ModeError> {
    let inactive_dur = Duration::new(inactive, 0);
    let active_dur = Duration::new(active, 0);
    loop {
        //assumes cc is not running on entry
        sleep(inactive_dur).await;
        match start_cc(main_camera_tx.clone()).await {
            Ok(_) => {}
            Err(e) => return Err(CameraRequestError),
        }
        sleep(active_dur).await;
        match end_cc(main_camera_tx.clone()).await {
            Ok(_) => {}
            Err(e) => return Err(CameraRequestError),
        }
    }
}

async fn pan_search(
    gimbal_positions: Vec<GimbalPosition>,
    gimbal_tx: Option<
        flume::Sender<(
            GimbalRequest,
            tokio::sync::oneshot::Sender<Result<GimbalResponse, Error>>,
        )>,
    >,
    main_camera_tx: flume::Sender<(
        CameraRequest,
        tokio::sync::oneshot::Sender<Result<CameraResponse, anyhow::Error>>,
    )>,
) -> Result<(), ModeError> {
    let gimbal_tx = match gimbal_tx {
        Some(gimbal_tx) => gimbal_tx,
        None => return Err(GimbalRequestError),
    };

    loop {
        for pos in &gimbal_positions {
            // pitch, roll
            match rotate_gimbal(pos.roll, pos.pitch, gimbal_tx.clone()).await {
                Ok(_) => {}
                Err(e) => return Err(GimbalRequestError),
            }
            match capture(main_camera_tx.clone()).await {
                Ok(_) => {}
                Err(e) => return Err(CameraRequestError),
            }
        }
    }
}

async fn distance_search(
    distance_threshold: u64,
    waypoints: Vec<geo::Point>,
    telemetry_rx: watch::Receiver<Telemetry>,
    main_camera_tx: flume::Sender<(
        CameraRequest,
        tokio::sync::oneshot::Sender<Result<CameraResponse, anyhow::Error>>,
    )>,
) -> Result<(), ModeError> {
    let mut enter = true; // start assuming not in range
    loop {
        match transition_by_distance(
            &waypoints[..],
            telemetry_rx.clone(),
            distance_threshold,
            enter,
        )
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
        match transition_by_distance(
            &waypoints[..],
            telemetry_rx.clone(),
            distance_threshold,
            enter,
        )
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
impl Task for ModesTask {
    fn name(&self) -> &'static str {
        "modes/control"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let loop_fut = async move {
            let ctrl_evt_tx = self.cmd_tx;
            loop {
                match self.cmd_rx.recv_async().await {
                    Ok((req, ret)) => {
                        let result: Result<ModeResponse, ModeError> = match req {
                            ModeRequest::Inactive => todo!(),
                            ModeRequest::LivestreamOnly => todo!(),
                            ModeRequest::ZoomControl(req) => todo!(),
                            ModeRequest::Search(req) => match req {
                                SearchRequest::Time { active, inactive } => {
                                    time_search(active, inactive, self.camera_ctrl_cmd_tx.clone())
                                        .await
                                        .map(|_| ModeResponse::Response)
                                }
                                SearchRequest::Distance { distance, waypoint } => distance_search(
                                    distance,
                                    waypoint,
                                    self.telem_rx.clone(),
                                    self.camera_ctrl_cmd_tx.clone(),
                                )
                                .await
                                .map(|_| ModeResponse::Response),
                                SearchRequest::Manual { start } if start => {
                                    start_cc(self.camera_ctrl_cmd_tx.clone())
                                        .await
                                        .map(|_| ModeResponse::Response)
                                        .map_err(ModeError::Camera)
                                }
                                SearchRequest::Manual { start } => {
                                    end_cc(self.camera_ctrl_cmd_tx.clone())
                                        .await
                                        .map(|_| ModeResponse::Response)
                                        .map_err(ModeError::Camera)
                                }
                                SearchRequest::Panning {} => match &self.gimbal_tx {
                                    None => {
                                        start_cc(self.camera_ctrl_cmd_tx.clone())
                                            .await
                                            .map(|_| ModeResponse::Response)
                                            .map_err(ModeError::Camera);
                                        sleep(Duration::new(180, 0)).await;
                                        end_cc(self.camera_ctrl_cmd_tx.clone())
                                            .await
                                            .map(|_| ModeResponse::Response)
                                            .map_err(ModeError::Camera)
                                    }
                                    _ => pan_search(
                                        self.modes_config.gimbal_positions.clone(),
                                        self.gimbal_tx.clone(),
                                        self.camera_ctrl_cmd_tx.clone(),
                                    )
                                    .await
                                    .map(|_| ModeResponse::Response),
                                },
                            },
                        };

                        let _ = ret.send(result);
                    }
                    Err(_) => break,
                }
            }

            Ok::<_, ModeError>(())
        };

        select! {
          _ = cancel.cancelled() => {}
          res = loop_fut => { res? }
        }

        Ok(())
    }
}
