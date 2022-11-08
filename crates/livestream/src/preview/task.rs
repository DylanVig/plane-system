use anyhow::{bail, Context, anyhow};
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
            debug!("initializing live preview");

            gst::init().context("failed to init gstreamer")?;

            let start_time = Local::now();

            let pipeline = gst::Pipeline::default();

            let appsrc = gst_app::AppSrc::builder()
                .caps(&gst::Caps::builder("image/jpeg").build())
                .format(gst::Format::Time)
                .is_live(true)
                .build();

            let jpegdec = gst::ElementFactory::make("jpegdec").build()?;
            let videoconvert = gst::ElementFactory::make("videoconvert").build()?;
            let sink = gst::ElementFactory::make("autovideosink").build()?;

            pipeline.add_many(&[appsrc.upcast_ref(), &jpegdec, &videoconvert, &sink])?;
            gst::Element::link_many(&[appsrc.upcast_ref(), &jpegdec, &videoconvert, &sink])?;

            let bus = pipeline.bus().context("failed to get element bus")?;
            let mut bus_stream = bus.stream();

            pipeline
                .set_state(gst::State::Playing)
                .context("could not play gstreamer pipeline")?;

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

                            appsrc.push_buffer(buf).context("pushing buffer failed")?;
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
                                pipeline.set_state(gst::State::Null)?;
                                return Err(anyhow!(err.error()));
                            }
                            _ => (),
                        }
                    }
                }
            }

            appsrc.end_of_stream().context("ending stream failed")?;

            pipeline
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
