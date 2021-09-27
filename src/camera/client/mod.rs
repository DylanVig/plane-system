use anyhow::Context;
use futures::FutureExt;
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

    let client = Arc::new(RwLock::new(CameraClient { interface, state }));
    let (ptp_tx, _) = broadcast::channel(256);

    info!("initialized camera");

    let download_fut = run_download(
        client.clone(),
        ptp_tx.subscribe(),
        channels.camera_event.clone(),
    );
    let cmd_fut = run_commands(client.clone(), ptp_tx.subscribe(), command_rx);

    let download_handle = tokio::spawn(download_fut);
    let cmd_handle = tokio::spawn(cmd_fut);
    let event_handle = tokio::task::spawn_blocking({
        let client = client.clone();
        move || run_events(client, ptp_tx, channels.interrupt.subscribe())
    });

    tokio::select! {
        download_res = download_handle => download_res??,
        event_res = event_handle => event_res??,
        cmd_res = cmd_handle => cmd_res??,
    };

    Ok(())
}

fn run_events(
    client: Arc<RwLock<CameraClient>>,
    events_ptp: broadcast::Sender<ptp::PtpEvent>,
    mut interrupt_rx: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    loop {
        let event = {
            trace!("event: locking client for read");
            let rt = tokio::runtime::Handle::current();
            let client = rt.block_on(client.read());
            trace!("event: locked client for read");

            trace!("ptp recv");
            client
                .interface
                .recv(Some(Duration::from_millis(500)))
                .context("error while receiving camera event")?
        };

        trace!("event: unlocked client for read");

        if let Some(event) = event {
            debug!("event: recv {:?}", event);

            if let Err(_) = events_ptp.send(event) {
                break;
            }
        }

        if let Ok(()) = interrupt_rx.try_recv() {
            break;
        }

        // tokio::task::yield_now().await;
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
            CameraCommandRequest::Debug(req) => cmd_debug(client.clone(), req).await,
            CameraCommandRequest::Capture => cmd_capture(client.clone(), &mut ptp_rx).await,
            CameraCommandRequest::ContinuousCapture(req) => {
                cmd_continuous_capture(client.clone(), req).await
            }
            CameraCommandRequest::Storage(_) => todo!(),
            CameraCommandRequest::File(_) => todo!(),
            CameraCommandRequest::Reconnect => todo!(),
            CameraCommandRequest::Zoom(_) => todo!(),
            CameraCommandRequest::Exposure(_) => todo!(),
            CameraCommandRequest::SaveMode(_) => todo!(),
            CameraCommandRequest::OperationMode(_) => todo!(),
            CameraCommandRequest::FocusMode(_) => todo!(),
            CameraCommandRequest::Record(_) => todo!(),
        };

        let _ = command.chan.send(result);
    }
}

async fn run_download(
    client: Arc<RwLock<CameraClient>>,
    mut ptp_rx: broadcast::Receiver<ptp::PtpEvent>,
    client_tx: broadcast::Sender<CameraClientEvent>,
) -> anyhow::Result<()> {
    let client = &*client;

    loop {
        wait(&mut ptp_rx, ptp::EventCode::Vendor(0xC203)).await?;

        let shooting_file_info = {
            trace!("downloader: locking client for read");
            let client = client.read().await;
            trace!("downloader: locked client for read");

            client
                .state
                .get(&CameraPropertyCode::ShootingFileInfo)
                .context("shooting file counter is unknown")?
                .current
                .clone()
        };

        trace!("downloader: unlocked client for read");

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
            let (info, data) = {
                trace!("downloader: locking client for write");
                let mut client = client.write().await;
                trace!("downloader: locked client for write");
                download(&mut client.interface, ptp::ObjectHandle::from(0xFFFFC001))?
            };

            trace!("downloader: unlocked client for write");

            let _ = client_tx.send(CameraClientEvent::Download {
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
