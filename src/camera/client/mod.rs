use anyhow::Context;
use num_traits::FromPrimitive;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{broadcast, RwLock};
use tokio::task::block_in_place;

use crate::Channels;

use super::interface::*;
use super::*;

mod command;
mod util;

use self::command::*;
use self::util::*;

const TIMEOUT: Duration = Duration::from_secs(1);

struct CameraClient {
    interface: CameraInterface,
    state: HashMap<CameraPropertyCode, ptp::PtpPropInfo>,
}

pub async fn run(
    channels: Arc<Channels>,
    command_rx: flume::Receiver<CameraCommand>,
) -> anyhow::Result<()> {
    let mut interface = CameraInterface::new().context("failed to create camera interface")?;

    interface.connect().context("failed to connect to camera")?;

    trace!("intializing camera");

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

    let client = Arc::new(RwLock::new(CameraClient { interface, state }));
    let (ptp_tx, _) = broadcast::channel(256);

    info!("initialized camera");

    let download_fut = run_download(
        client.clone(),
        ptp_tx.subscribe(),
        channels.camera_event.clone(),
    );
    let cmd_fut = run_commands(client.clone(), ptp_tx.subscribe(), command_rx);
    let event_fut = run_events(client.clone(), ptp_tx);

    let results = futures::join!(event_fut, cmd_fut, download_fut);

    results.0?;
    results.1?;
    results.2?;

    Ok(())
}

async fn run_events(
    client: Arc<RwLock<CameraClient>>,
    events_ptp: broadcast::Sender<ptp::PtpEvent>,
) -> anyhow::Result<()> {
    loop {
        let interface = &client.read().await.interface;
        let event = block_in_place(|| interface.recv(Some(Duration::from_millis(10))))
            .context("error while receiving camera event")?;

        if let Some(event) = event {
            if let Err(_) = events_ptp.send(event) {
                break;
            }
        }
    }

    Ok(())
}

async fn run_commands(
    client: Arc<RwLock<CameraClient>>,
    mut ptp_rx: broadcast::Receiver<ptp::PtpEvent>,
    command_rx: flume::Receiver<CameraCommand>,
) -> anyhow::Result<()> {
    loop {
        let command = command_rx.recv_async().await?;

        let result = match command.request {
            CameraCommandRequest::Storage(_) => todo!(),
            CameraCommandRequest::File(_) => todo!(),
            CameraCommandRequest::Capture => cmd_capture(client.clone(), &mut ptp_rx).await,
            CameraCommandRequest::ContinuousCapture(req) => {
                cmd_continuous_capture(client.clone(), req).await
            }
            CameraCommandRequest::Power(_) => todo!(),
            CameraCommandRequest::Reconnect => todo!(),
            CameraCommandRequest::Zoom(_) => todo!(),
            CameraCommandRequest::Exposure(_) => todo!(),
            CameraCommandRequest::SaveMode(_) => todo!(),
            CameraCommandRequest::OperationMode(_) => todo!(),
            CameraCommandRequest::FocusMode(_) => todo!(),
            CameraCommandRequest::Record(_) => todo!(),
            CameraCommandRequest::Debug(req) => cmd_debug(client.clone(), req).await,
        };

        let _ = command.chan.send(result);
    }
}

async fn run_download(
    client: Arc<RwLock<CameraClient>>,
    mut ptp_rx: broadcast::Receiver<ptp::PtpEvent>,
    client_tx: broadcast::Sender<CameraEvent>,
) -> anyhow::Result<()> {
    let client = &*client;

    loop {
        wait(&mut ptp_rx, ptp::EventCode::Vendor(0xC204)).await?;

        let shooting_file_info = client
            .read()
            .await
            .state
            .get(&CameraPropertyCode::ShootingFileInfo)
            .context("shooting file counter is unknown")?
            .current
            .clone();

        let mut shooting_file_info = match shooting_file_info {
            ptp::PtpData::UINT16(shooting_file_info) => shooting_file_info,
            _ => panic!("shooting file info is not a u16"),
        };

        debug!(
            "received shooting file info confirmation; current value = {:04x}",
            shooting_file_info
        );

        // let _ = client_tx.send(CameraEvent::Capture);

        while shooting_file_info > 0 {
            let (info, data) = download(
                &mut client.write().await.interface,
                ptp::ObjectHandle::from(0xFFFFC001),
            )
            .await?;

            let _ = client_tx.send(CameraEvent::Download {
                image_name: info.filename,
                image_data: Arc::new(data),
            });

            let (new, _) = watch(client, CameraPropertyCode::ShootingFileInfo).await?;

            shooting_file_info = match new {
                ptp::PtpData::UINT16(new) => new,
                _ => panic!("shooting file info is not a u16"),
            };
        }
    }
}
