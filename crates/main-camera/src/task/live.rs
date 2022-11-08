use std::{sync::Arc, time::Duration};

use anyhow::Context;
use async_trait::async_trait;
use bytes::{Buf, Bytes, BytesMut};
use log::*;

use ps_client::Task;

use tokio::{io::AsyncWriteExt, select, sync::RwLock, time::interval};
use tokio_util::sync::CancellationToken;

use crate::{interface::PropertyCode, task::util::convert_camera_value};

use super::InterfaceGuard;

/// This is the object handle for images stored in the image buffer. The image
/// buffer is used when the camera does not contain an SD card, and is used to
/// retrieve images that are stored temporarily on the camera after capture.
const IMAGE_BUFFER_OBJECT_HANDLE: u32 = 0xFFFFC002;

#[derive(Clone, Debug)]
pub struct LiveFrame(Bytes);

pub struct LiveTask {
    interface: Arc<RwLock<InterfaceGuard>>,

    frame_tx: flume::Sender<LiveFrame>,
    frame_rx: flume::Receiver<LiveFrame>,
}

impl LiveTask {
    pub(super) fn new(interface: Arc<RwLock<InterfaceGuard>>) -> Self {
        let (frame_tx, frame_rx) = flume::bounded(256);

        Self {
            interface,
            frame_rx,
            frame_tx,
        }
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
            interface,
            frame_tx,
            ..
        } = *self;

        let loop_fut = async move {
            let mut ival = interval(Duration::from_secs_f32(1.0 / 15.0));

            loop {
                ival.tick().await;

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
                    let data = interface
                        .object_data(handle, None)
                        .context("failed to get data for image")?;

                    Ok::<_, anyhow::Error>(data)
                })?;

                let mut lv_data = BytesMut::from(&lv_data[..]);

                trace!("downloaded live frame camera");

                // data is encoded as (offset, len, buffer)
                // where buffer contains a jpeg image at given offset w/ given length
                let offset = lv_data.get_u32_le() as usize;
                let size = lv_data.get_u32_le() as usize;
                let image = lv_data.split_off(offset - 8).split_to(size);

                let _ = frame_tx.try_send(LiveFrame(image.freeze()));
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
