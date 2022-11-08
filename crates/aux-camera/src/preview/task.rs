use anyhow::{bail, Context};
use async_trait::async_trait;
use chrono::Local;
use gst::traits::ElementExt;
use gst::{
    glib::value::{FromValue, ValueTypeChecker},
    prelude::{ObjectExt, ToValue},
};
use log::*;
use ps_client::{ChannelCommandSink, ChannelCommandSource, Task};
use tokio::select;
use tokio_util::sync::CancellationToken;

use super::*;

pub struct PreviewTask {
    config: PreviewConfig,
    frame_rx: flume::Receiver<ps_main_camera::LiveFrame>,
}

pub fn create_task(
    config: PreviewConfig,
    frame_rx: flume::Receiver<ps_main_camera::LiveFrame>,
) -> anyhow::Result<PreviewTask> {
    Ok(PreviewTask { config, frame_rx })
}

#[async_trait]
impl Task for PreviewTask {
    fn name(&self) -> &'static str {
        "live/preview"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self { frame_rx, .. } = *self;

        let cmd_loop = async {
            trace!("initializing live preview");

            gst::init().context("failed to init gstreamer")?;

            let start_time = Local::now();

            let element = gst::parse_launch("appsrc ! jpegdec ! autovideosink")
                .context("failed to create gstreamer pipeline")?;

            element.set_property("caps", gst::Caps::builder("image/jpeg").build());
            element.set_property("is-live", true);
            element.set_property("emit-signals", true);

            let bus = element.bus().context("failed to get element bus")?;

            element
                .set_state(gst::State::Playing)
                .context("could not play gstreamer pipeline")?;

            while let Ok(frame) = frame_rx.recv_async().await {
                let mut buf = gst::Buffer::with_size(frame.data.len())
                    .context("failed to allocate gstreamer framebuffer")?;
                {
                    let buf = buf
                        .get_mut()
                        .context("failed to write to gstreamer framebuffer")?;

                    if let Err(_) = buf.copy_from_slice(0, &frame.data[..]) {
                        bail!("failed to fill gstreamer framebuffer");
                    }

                    let frame_time = frame.timestamp - start_time;
                    buf.set_dts(gst::ClockTime::from_mseconds(
                        frame_time.num_milliseconds() as u64
                    ));
                }

                let ret = bus
                    .emit_by_name_with_values("push-buffer", &[buf.to_value()])
                    .context("pushing buffer returned nothing")?;

                <gst::FlowReturn as FromValue>::Checker::check(&ret)
                    .context("push-buffer return value was not a FlowReturn")?;
                let ret = unsafe { gst::FlowReturn::from_value(&ret) };

                match ret {
                    gst::FlowReturn::Ok => {}
                    other => bail!("pushing buffer returned {other:?}"),
                }
            }

            bus.emit_by_name_with_values("end-of-stream", &[]);

            element
                .set_state(gst::State::Null)
                .context("could not stop gstreamer pipeline")?;

            Ok::<_, anyhow::Error>(())
        };

        select! {
            _ = cancel.cancelled() => {}
            res = cmd_loop => { res? }
        };

        Ok(())
    }
}
