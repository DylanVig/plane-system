use anyhow::Context;
use futures::{Future, FutureExt};
use num_traits::FromPrimitive;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{broadcast, oneshot, OwnedSemaphorePermit, RwLock, Semaphore, SemaphorePermit};
use tokio::task::block_in_place;

use crate::Channels;

use super::interface::*;
use super::*;

mod command;
mod util;

use self::command::*;
use self::util::*;

const TIMEOUT: Duration = Duration::from_secs(5);

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

    let download_task = tokio::spawn(run_download(
        interface_req_buf.clone(),
        ptp_tx.subscribe(),
        channels.camera_event.clone(),
    ));

    task_names.push("download");
    futures.push(download_task);

    let cmd_task = tokio::spawn(run_commands(
        interface_req_buf.clone(),
        ptp_tx.subscribe(),
        command_rx,
    ));

    task_names.push("cmd");
    futures.push(cmd_task);

    let interface_task = tokio::task::spawn_blocking({
        let interface = interface.clone();
        let interrupt_rx = channels.interrupt.subscribe();
        move || run_interface(interface, state, interface_rx, interrupt_rx)
    });

    task_names.push("interface");
    futures.push(interface_task);

    let event_task = tokio::task::spawn_blocking({
        let interface = interface.clone();
        let interrupt_rx = channels.interrupt.subscribe();
        move || run_events(interface, semaphore, ptp_tx, interrupt_rx)
    });

    task_names.push("event");
    futures.push(event_task);

    while futures.len() > 0 {
        // wait for each task to end
        let (result, i, remaining) = futures::future::select_all(futures).await;
        let task_name = task_names.remove(i);

        info!(
            "{} ({}) task ended, {} remaining",
            task_name,
            i,
            task_names.join(", ")
        );

        // if a task ended with an error or did not join properly, end the process
        // with an interrupt
        if let Err(err) = result? {
            error!(
                "got error from {} task, sending interrupt: {:?}",
                task_name, err
            );

            info!("remaining tasks: {:?}", task_names.join(", "));
        }

        futures = remaining;
    }

    Ok(())
}

#[derive(Debug)]
enum CameraInterfaceRequest {
    GetPropertyInfo {
        property: CameraPropertyCode,
        ret: oneshot::Sender<Option<ptp::PtpPropInfo>>,
    },
    GetPropertyValue {
        property: CameraPropertyCode,
        ret: oneshot::Sender<Option<ptp::PtpData>>,
    },
    SetPropertyValue {
        property: CameraPropertyCode,
        value: ptp::PtpData,
        ret: oneshot::Sender<anyhow::Result<()>>,
    },
    UpdatePropertyValues {
        ret: oneshot::Sender<anyhow::Result<()>>,
    },
    Control {
        control: CameraControlCode,
        data: ptp::PtpData,
        ret: oneshot::Sender<anyhow::Result<()>>,
    },
    StorageIds {
        ret: oneshot::Sender<anyhow::Result<Vec<ptp::StorageId>>>,
    },
    StorageInfo {
        handle: ptp::StorageId,
        ret: oneshot::Sender<anyhow::Result<ptp::PtpStorageInfo>>,
    },
    ObjectInfo {
        handle: ptp::ObjectHandle,
        ret: oneshot::Sender<anyhow::Result<ptp::PtpObjectInfo>>,
    },
    ObjectData {
        handle: ptp::ObjectHandle,
        ret: oneshot::Sender<anyhow::Result<Vec<u8>>>,
    },
}

fn run_interface(
    interface: Arc<CameraInterface>,
    mut state: HashMap<CameraPropertyCode, ptp::PtpPropInfo>,
    req_rx: flume::Receiver<CameraInterfaceRequest>,
    mut interrupt_rx: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    loop {
        let req = req_rx.recv()?;

        match req {
            CameraInterfaceRequest::GetPropertyInfo { property, ret } => {
                let _ = ret.send(state.get(&property).cloned());
            }
            CameraInterfaceRequest::GetPropertyValue { property, ret } => {
                let _ = ret.send(state.get(&property).map(|pi| pi.current.clone()));
            }
            CameraInterfaceRequest::SetPropertyValue {
                property,
                value: action,
                ret,
            } => {
                let _ = ret.send(interface.set(property, action));
            }
            CameraInterfaceRequest::UpdatePropertyValues { ret } => match interface.update() {
                Ok(properties) => {
                    for property in properties {
                        if let Some(property_code) =
                            <CameraPropertyCode as FromPrimitive>::from_u16(property.property_code)
                        {
                            state.insert(property_code, property);
                        }
                    }

                    let _ = ret.send(Ok(()));
                }
                Err(err) => {
                    let _ = ret.send(Err(err));
                }
            },
            CameraInterfaceRequest::Control {
                control,
                data: action,
                ret,
            } => {
                let _ = ret.send(interface.execute(control, action));
            }
            CameraInterfaceRequest::StorageIds { ret } => {
                let _ = ret.send(interface.storage_ids(Some(TIMEOUT)));
            }
            CameraInterfaceRequest::StorageInfo { handle, ret } => {
                let _ = ret.send(interface.storage_info(handle, Some(TIMEOUT)));
            }
            CameraInterfaceRequest::ObjectInfo { handle, ret } => {
                let _ = ret.send(interface.object_info(handle, Some(TIMEOUT)));
            }
            CameraInterfaceRequest::ObjectData { handle, ret } => {
                let _ = ret.send(interface.object_data(handle, Some(TIMEOUT)));
            }
        }

        if let Ok(()) = interrupt_rx.try_recv() {
            debug!("camera interface runner interrupted");
            break Ok(());
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

fn run_events(
    interface: Arc<CameraInterface>,
    semaphore: Arc<Semaphore>,
    events_ptp: broadcast::Sender<ptp::PtpEvent>,
    mut interrupt_rx: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let rt_handle = tokio::runtime::Handle::current();

    loop {
        let event = {
            // let sem = rt_handle
            //     .block_on(semaphore.acquire())
            //     .context("error while acquiring interface semaphore")
            //     .unwrap();

            interface
                .recv(None)
                .context("error while receiving camera event")?
        };

        if let Some(event) = event {
            debug!("event: recv {:?}", event);

            if let Err(_) = events_ptp.send(event) {
                debug!("camera event querier exited");
                break;
            }
        }

        if let Ok(()) = interrupt_rx.try_recv() {
            debug!("camera event querier interrupted");
            break;
        }
    }

    Ok(())
}

#[derive(Clone)]
struct CameraInterfaceRequestBuffer {
    chan: flume::Sender<CameraInterfaceRequest>,
    semaphore: Arc<Semaphore>,
}

impl CameraInterfaceRequestBuffer {
    pub async fn enter<
        T,
        Fut: Future<Output = T>,
        F: FnOnce(CameraInterfaceRequestBufferGuard) -> Fut,
    >(
        &self,
        f: F,
    ) -> T {
        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("could not acquire camera interface request buffer semaphore");
        let guard = CameraInterfaceRequestBufferGuard(self.chan.clone(), permit);
        trace!("entered guard");
        let result = f(guard).await;
        trace!("exited guard");
        result
    }
}

struct CameraInterfaceRequestBufferGuard(
    flume::Sender<CameraInterfaceRequest>,
    OwnedSemaphorePermit,
);

impl CameraInterfaceRequestBufferGuard {
    pub async fn get_info(&self, property: CameraPropertyCode) -> Option<ptp::PtpPropInfo> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send_async(CameraInterfaceRequest::GetPropertyInfo { property, ret: tx })
            .await
            .unwrap();
        rx.await.unwrap()
    }

    pub async fn get_value(&self, property: CameraPropertyCode) -> Option<ptp::PtpData> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send_async(CameraInterfaceRequest::GetPropertyValue { property, ret: tx })
            .await
            .unwrap();
        rx.await.unwrap()
    }

    pub async fn set_value(
        &self,
        property: CameraPropertyCode,
        value: ptp::PtpData,
    ) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send_async(CameraInterfaceRequest::SetPropertyValue {
                property,
                value,
                ret: tx,
            })
            .await
            .unwrap();
        rx.await.unwrap()
    }

    pub async fn update(&self) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send_async(CameraInterfaceRequest::UpdatePropertyValues { ret: tx })
            .await
            .unwrap();
        rx.await.unwrap()
    }

    pub async fn control(
        &self,
        control: CameraControlCode,
        data: ptp::PtpData,
    ) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send_async(CameraInterfaceRequest::Control {
                control,
                data,
                ret: tx,
            })
            .await
            .unwrap();
        rx.await.unwrap()
    }

    pub async fn storage_ids(&self) -> anyhow::Result<Vec<ptp::StorageId>> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send_async(CameraInterfaceRequest::StorageIds { ret: tx })
            .await
            .unwrap();
        rx.await.unwrap()
    }

    pub async fn storage_info(
        &self,
        handle: ptp::StorageId,
    ) -> anyhow::Result<ptp::PtpStorageInfo> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send_async(CameraInterfaceRequest::StorageInfo { handle, ret: tx })
            .await
            .unwrap();
        rx.await.unwrap()
    }

    pub async fn object_info(
        &self,
        handle: ptp::ObjectHandle,
    ) -> anyhow::Result<ptp::PtpObjectInfo> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send_async(CameraInterfaceRequest::ObjectInfo { handle, ret: tx })
            .await
            .unwrap();
        rx.await.unwrap()
    }

    pub async fn object_data(&self, handle: ptp::ObjectHandle) -> anyhow::Result<Vec<u8>> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send_async(CameraInterfaceRequest::ObjectData { handle, ret: tx })
            .await
            .unwrap();
        rx.await.unwrap()
    }
}

async fn run_commands(
    interface: CameraInterfaceRequestBuffer,
    mut ptp_rx: broadcast::Receiver<ptp::PtpEvent>,
    command_rx: flume::Receiver<CameraCommand>,
) -> anyhow::Result<()> {
    loop {
        let command = command_rx.recv_async().await?;

        let fut = match command.request {
            CameraCommandRequest::Debug(req) => cmd_debug(interface.clone(), req),
            CameraCommandRequest::Capture => cmd_capture(interface.clone(), &mut ptp_rx),
            CameraCommandRequest::ContinuousCapture(req) => {
                cmd_continuous_capture(interface.clone(), req)
            }
            CameraCommandRequest::Storage(req) => cmd_storage(interface.clone(), req),
            CameraCommandRequest::File(_) => todo!(),
            CameraCommandRequest::Reconnect => todo!(),
            CameraCommandRequest::Zoom(_) => todo!(),
            CameraCommandRequest::Exposure(_) => todo!(),
            CameraCommandRequest::SaveMode(_) => todo!(),
            CameraCommandRequest::OperationMode(_) => todo!(),
            CameraCommandRequest::FocusMode(_) => todo!(),
            CameraCommandRequest::Record(_) => todo!(),
        };

        let result = fut.await;

        let _ = command.chan.send(result);
    }
}

async fn run_download(
    interface: CameraInterfaceRequestBuffer,
    mut ptp_rx: broadcast::Receiver<ptp::PtpEvent>,
    client_tx: broadcast::Sender<CameraClientEvent>,
) -> anyhow::Result<()> {
    loop {
        wait(&mut ptp_rx, ptp::EventCode::Vendor(0xC204)).await?;

        tokio::time::sleep(Duration::from_millis(500)).await;

        let shooting_file_info = interface
            .enter(|i| async move {
                i.update().await?;
                Ok::<_, anyhow::Error>(i.get_value(CameraPropertyCode::ShootingFileInfo).await)
            })
            .await?;

        let mut shooting_file_info = match shooting_file_info {
            Some(ptp::PtpData::UINT16(shooting_file_info)) => shooting_file_info,
            _ => panic!("shooting file info is not a u16"),
        };

        // let _ = client_tx.send(CameraEvent::Capture);

        while shooting_file_info & 0x8000 != 0 {
            debug!(
                "received shooting file info confirmation; current value = {:04x}",
                shooting_file_info
            );

            tokio::time::sleep(Duration::from_millis(500)).await;

            let result = interface
                .enter(|i| async move {
                    let info = i
                        .object_info(ptp::ObjectHandle::from(0xFFFFC001))
                        .await
                        .context("failed to get object info for download")?;

                    let data = i
                        .object_data(ptp::ObjectHandle::from(0xFFFFC001))
                        .await
                        .context("failed to get object data for download")?;

                    Ok::<_, anyhow::Error>((info, data))
                })
                .await;

            let (info, data) = match result {
                Ok(result) => result,
                Err(err) => {
                    warn!("downloading image data failed: {:?}", err);
                    continue;
                }
            };

            let _ = client_tx.send(CameraClientEvent::Download {
                image_name: info.filename,
                image_data: Arc::new(data),
            });

            let (new, _) = watch(&interface, CameraPropertyCode::ShootingFileInfo).await?;

            shooting_file_info = match new {
                ptp::PtpData::UINT16(new) => new,
                _ => panic!("shooting file info is not a u16"),
            };
        }

        debug!(
            "received shooting file info confirmation; current value = {:04x}",
            shooting_file_info
        );

        info!("download complete");
    }
}
