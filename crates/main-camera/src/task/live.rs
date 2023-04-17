use std::{sync::Arc, time::Duration};

use anyhow::{bail, Context};
use async_trait::async_trait;
use bytes::{Buf, Bytes, BytesMut};
use chrono::{DateTime, Local};
use log::*;

use ps_client::Task;

use tokio::{select, sync::RwLock, time::interval};
use tokio_util::sync::CancellationToken;

use crate::{interface::PropertyCode, task::util::convert_camera_value, LiveConfig};

use super::InterfaceGuard;

/// This is the object handle for images stored in the image buffer. The image
/// buffer is used when the camera does not contain an SD card, and is used to
/// retrieve images that are stored temporarily on the camera after capture.
const IMAGE_BUFFER_OBJECT_HANDLE: u32 = 0xFFFFC002;

#[derive(Clone, Debug)]
pub struct LiveFrame {
    pub timestamp: DateTime<Local>,

    /// Frame data encoded as JPEG
    pub data: Bytes,
}

pub struct LiveTask {
    interface: Arc<RwLock<InterfaceGuard>>,
    config: LiveConfig,
    frame_tx: flume::Sender<LiveFrame>,
    frame_rx: flume::Receiver<LiveFrame>,
}

impl LiveTask {
    pub(super) fn new(
        interface: Arc<RwLock<InterfaceGuard>>,
        config: LiveConfig,
    ) -> anyhow::Result<Self> {
        let (frame_tx, frame_rx) = flume::bounded(256);

        if config.framerate <= 0.0 || config.framerate > 30.0 {
            bail!("camera live preview framerate must be greater than zero and less than or equal to 30");
        }

        Ok(Self {
            interface,
            config,
            frame_rx,
            frame_tx,
        })
    }

    pub fn frame(&self) -> flume::Receiver<LiveFrame> {
        self.frame_rx.clone()
    }
}

#[async_trait]
impl Task for LiveTask {
    fn name(&self) -> &'static str {
        "main-camera/live"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            config,
            interface,
            frame_tx,
            ..
        } = *self;

        let loop_fut = async move {
            let frame_duration = Duration::from_secs_f32(1.0 / config.framerate);
            let mut interval = interval(frame_duration);

            loop {
                interval.tick().await;

                let mut interface = interface.write().await;
                let props = interface.query().context("could not get camera state")?;
                let lv_status: u8 = convert_camera_value(&props, PropertyCode::LiveViewStatus)
                    .context("could not get live view status")?;

                match lv_status {
                    0x00 => {
                        debug!("live view disabled, trying again");
                        continue;
                    }
                    0x01 => {
                        trace!("live view enabled, retrieving frame");
                    }
                    0x02 => {
                        error!("live view not supported, exiting live view task");
                        break;
                    }
                    other => {
                        error!("unknown live view status {other:#x}");
                        break;
                    }
                }

                let lv_data = tokio::task::block_in_place(|| {
                    let handle = ptp::ObjectHandle::from(IMAGE_BUFFER_OBJECT_HANDLE);
                    let data = match interface.get_object(handle, None) {
                        Ok(data) => Some(data),

                        // Sony says that this will sometimes fail with
                        // AccessDenied, but in that case we should just try
                        // again
                        Err(ptp::Error::Response(ptp::ResponseCode::Standard(
                            ptp::StandardResponseCode::AccessDenied,
                        ))) => None,

                        Err(err) => return Err(err).context("failed to retrieve live view frame"),
                    };

                    Ok::<_, anyhow::Error>(data)
                })?;

                let lv_data = match lv_data {
                    Some(lv_data) => lv_data,
                    None => continue,
                };

                let mut lv_data = BytesMut::from(&lv_data[..]);

                trace!("downloaded live frame camera");

                // data is encoded as (offset, len, buffer)
                // where buffer contains a jpeg image at given offset w/ given length
                let offset = lv_data.get_u32_le() as usize;
                let size = lv_data.get_u32_le() as usize;
                let image = lv_data.split_off(offset - 8).split_to(size);

                let _ = frame_tx.try_send(LiveFrame {
                    timestamp: Local::now(),
                    data: image.freeze(),
                });
            }

            #[allow(unreachable_code)]
            Ok::<_, anyhow::Error>(())
        };

        select! {
          _ = cancel.cancelled() => {}
          res = loop_fut => { res? }
        }

        Ok(())
    }
}
