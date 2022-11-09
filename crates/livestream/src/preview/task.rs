use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use async_trait::async_trait;
use chrono::Local;
use futures::StreamExt;
use gst::Pipeline;
use gst::{
    glib::value::{FromValue, ValueTypeChecker},
    prelude::*,
    traits::ElementExt,
};
use log::*;
use ps_client::{ChannelCommandSink, ChannelCommandSource, Task};
use tokio::select;
use tokio::time::sleep;
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
        let Self {
            config, frame_rx, ..
        } = *self;

        debug!("initializing live preview");

        gst::init().context("failed to init gstreamer")?;

        let start_time = Local::now();

        let pipeline = gst::Pipeline::default();

        let appsrc = gst_app::AppSrc::builder()
            .name("r10csrc")
            .caps(&gst::Caps::builder("image/jpeg").build())
            .format(gst::Format::Time)
            .is_live(true)
            .build();

        let bin_spec = config.bin_spec.join("\n");
        let bin = gst::parse_bin_from_description(&bin_spec, true)
            .context("failed to parse gstreamer bin from config")?;

        pipeline.add_many(&[appsrc.upcast_ref(), bin.upcast_ref::<gst::Element>()])?;
        gst::Element::link_many(&[appsrc.upcast_ref(), bin.upcast_ref::<gst::Element>()])?;

        pipeline
            .set_state(gst::State::Ready)
            .context("could not prepare gstreamer pipeline")?;

        let bus = pipeline.bus().context("failed to get element bus")?;
        let mut bus_stream = bus.stream();

        pipeline
            .set_state(gst::State::Playing)
            .context("could not prepare gstreamer pipeline")?;

        loop {
            select! {
                frame = frame_rx.recv_async() => {
                    if let Ok(frame) = frame {
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

                        if let Err(err) = appsrc.push_buffer(buf) {
                            match err {
                                // ignore flushing error
                                // gst::FlowError::Flushing => {
                                //     trace!("dropping frame b/c app source is flushing");
                                // }
                                err => {
                                    return Err(anyhow!(err)).context("failed to push buffer to appsrc");
                                }
                            }
                        }


                        trace!("pushed buffer to appsrc");
                    } else {
                        debug!("failed to get preview frame, exiting loop");
                        break;
                    }
                }

                msg = bus_stream.next() => {
                    use gst::MessageView;

                    if msg.is_none() {
                        debug!("message stream ended");
                        break;
                    }

                    match msg.unwrap().view() {
                        MessageView::Eos(..) => {
                            debug!("received eos from gstreamer");
                            break
                        },
                        MessageView::Error(err) => {
                            return Err(anyhow!(err.error()));
                        }
                        _ => (),
                    }
                }

                _ = cancel.cancelled() => {
                    break;
                }
            }
        }

        appsrc.end_of_stream().context("ending stream failed")?;
        pipeline
            .set_state(gst::State::Null)
            .context("error while stopping pipeline")?;
        sleep(Duration::from_millis(500)).await;

        Ok(())
    }
}
