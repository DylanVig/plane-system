use anyhow::Context;
use futures::future::Either;
use futures::Future;
use num_traits::FromPrimitive;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::{broadcast, oneshot, OwnedSemaphorePermit, Semaphore};
use tracing::Level;

use crate::util::spawn_blocking_with_name;
use crate::util::spawn_with_name;
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
        storage: ptp::StorageId,
        ret: oneshot::Sender<anyhow::Result<ptp::PtpStorageInfo>>,
    },
    ObjectHandles {
        storage: ptp::StorageId,
        parent_object: ptp::ObjectHandle,
        ret: oneshot::Sender<anyhow::Result<Vec<ptp::ObjectHandle>>>,
    },
    ObjectInfo {
        object: ptp::ObjectHandle,
        ret: oneshot::Sender<anyhow::Result<ptp::PtpObjectInfo>>,
    },
    ObjectData {
        object: ptp::ObjectHandle,
        ret: oneshot::Sender<anyhow::Result<Vec<u8>>>,
    },
}

fn run_interface(
    interface: Arc<CameraInterface>,
    mut state: HashMap<CameraPropertyCode, ptp::PtpPropInfo>,
    req_rx: flume::Receiver<CameraInterfaceRequest>,
    mut interrupt_rx: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let span = tracing::span!(Level::TRACE, "run_interface");
    let _enter = span.enter();

    let rt = tokio::runtime::Handle::current();

    while let Either::Left((Ok(req), _)) = rt.block_on(futures::future::select(
        Box::pin(req_rx.recv_async()),
        Box::pin(interrupt_rx.recv()),
    )) {
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

                    trace!("updated property values: {:#?}", state);

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
            CameraInterfaceRequest::StorageInfo {
                storage: handle,
                ret,
            } => {
                let _ = ret.send(interface.storage_info(handle, Some(TIMEOUT)));
            }
            CameraInterfaceRequest::ObjectHandles {
                storage,
                parent_object,
                ret,
            } => {
                let _ =
                    ret.send(interface.object_handles(storage, Some(parent_object), Some(TIMEOUT)));
            }
            CameraInterfaceRequest::ObjectInfo {
                object: handle,
                ret,
            } => {
                let _ = ret.send(interface.object_info(handle, Some(TIMEOUT)));
            }
            CameraInterfaceRequest::ObjectData {
                object: handle,
                ret,
            } => {
                let _ = ret.send(interface.object_data(handle, Some(TIMEOUT)));
            }
        }
    }

    Ok(())
}

fn run_events(
    interface: Arc<CameraInterface>,
    _semaphore: Arc<Semaphore>,
    events_ptp: broadcast::Sender<ptp::PtpEvent>,
    mut interrupt_rx: broadcast::Receiver<()>,
) -> anyhow::Result<()> {
    let _rt_handle = tokio::runtime::Handle::current();

    loop {
        let event = {
            // let sem = rt_handle
            //     .block_on(semaphore.acquire())
            //     .context("error while acquiring interface semaphore")
            //     .unwrap();

            interface
                .recv(Some(Duration::from_millis(100)))
                .context("error while receiving camera event")?
        };

        if let Some(event) = event {
            debug!("event: recv {:?}", event);

            if let Err(_) = events_ptp.send(event) {
                break;
            }
        }

        if let Ok(()) = interrupt_rx.try_recv() {
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

impl std::fmt::Debug for CameraInterfaceRequestBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CameraInterfaceRequestBuffer").finish()
    }
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
            .send_async(CameraInterfaceRequest::StorageInfo {
                storage: handle,
                ret: tx,
            })
            .await
            .unwrap();
        rx.await.unwrap()
    }

    pub async fn object_handles(
        &self,
        storage: ptp::StorageId,
        parent_object: ptp::ObjectHandle,
    ) -> anyhow::Result<Vec<ptp::ObjectHandle>> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send_async(CameraInterfaceRequest::ObjectHandles {
                storage,
                parent_object,
                ret: tx,
            })
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
            .send_async(CameraInterfaceRequest::ObjectInfo {
                object: handle,
                ret: tx,
            })
            .await
            .unwrap();
        rx.await.unwrap()
    }

    pub async fn object_data(&self, handle: ptp::ObjectHandle) -> anyhow::Result<Vec<u8>> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send_async(CameraInterfaceRequest::ObjectData {
                object: handle,
                ret: tx,
            })
            .await
            .unwrap();
        rx.await.unwrap()
    }
}

#[tracing::instrument]
async fn run_commands(
    interface: CameraInterfaceRequestBuffer,
    mut ptp_rx: broadcast::Receiver<ptp::PtpEvent>,
    command_rx: flume::Receiver<CameraCommand>,
    client_tx: broadcast::Sender<CameraClientEvent>,
) -> anyhow::Result<()> {
    loop {
        let command = command_rx.recv_async().await?;

        let result = match command.request {
            CameraCommandRequest::Capture => cmd_capture(interface.clone(), &mut ptp_rx).await,
            CameraCommandRequest::ContinuousCapture(req) => {
                cmd_continuous_capture(interface.clone(), req).await
            }
            CameraCommandRequest::Storage(req) => cmd_storage(interface.clone(), req).await,
            CameraCommandRequest::File(req) => {
                cmd_file(interface.clone(), req, client_tx.clone()).await
            }
            CameraCommandRequest::Reconnect => todo!(),
            CameraCommandRequest::Status => cmd_status(interface.clone()).await,
            CameraCommandRequest::Get(req) => cmd_get(interface.clone(), req).await,
            CameraCommandRequest::Set(req) => cmd_set(interface.clone(), req).await,
            CameraCommandRequest::Record(_) => todo!(),
        };

        let _ = command.chan.send(result);
    }
}

#[tracing::instrument]
async fn run_download(
    interface: CameraInterfaceRequestBuffer,
    mut ptp_rx: broadcast::Receiver<ptp::PtpEvent>,
    client_tx: broadcast::Sender<CameraClientEvent>,
) -> anyhow::Result<()> {
    loop {
        wait(&mut ptp_rx, ptp::EventCode::Vendor(0xC204)).await?;

        let event_timestamp = chrono::Local::now();

        let _ = client_tx.send(CameraClientEvent::Capture {
            timestamp: event_timestamp.clone(),
        });

        debug!("received camera capture event");

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
                cc_timestamp: Some(event_timestamp),
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
