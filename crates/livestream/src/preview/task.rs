use std::collections::HashMap;
use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use async_trait::async_trait;
use chrono::Local;
use futures::StreamExt;

use gst::{prelude::*, traits::ElementExt};
use log::*;
use ps_client::Task;
use tokio::select;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use super::*;

pub struct PreviewTask {
    bin_spec: String,
    frame_rx: flume::Receiver<ps_main_camera::LiveFrame>,
}

pub fn create_task(
    config: PreviewConfig,
    frame_rx: flume::Receiver<ps_main_camera::LiveFrame>,
) -> anyhow::Result<PreviewTask> {
    let mut save_path = config.save_path;
    save_path.push(chrono::Local::now().format("%FT%H-%M-%S").to_string());

    let mut fmt_vars = HashMap::new();
    fmt_vars.insert("save_path".to_owned(), save_path.display().to_string());

    let bin_spec = config.bin.join("\n");
    let bin_spec =
        strfmt::strfmt(&bin_spec, &fmt_vars).context("invalid pipeline format string")?;

    Ok(PreviewTask { bin_spec, frame_rx })
}

#[async_trait]
impl Task for PreviewTask {
    fn name(&self) -> &'static str {
        "live/preview"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            bin_spec, frame_rx, ..
        } = *self;

        debug!("initializing live preview");

        gst::init().context("failed to init gstreamer")?;

        let start_time = Local::now();

        let pipeline = gst::Pipeline::default();

        // create app source to put frames from camera into gstreamer
        let appsrc = gst_app::AppSrc::builder()
            .name("r10csrc")
            .caps(&gst::Caps::builder("image/jpeg").build())
            .format(gst::Format::Time)
            .is_live(true)
            .build();

        // create bin based on spec from config file
        let bin = gst::parse_bin_from_description(&bin_spec, true)
            .context("failed to parse gstreamer bin from config")?;

        // link the bin to the app source
        pipeline.add_many(&[appsrc.upcast_ref(), bin.upcast_ref::<gst::Element>()])?;
        gst::Element::link_many(&[appsrc.upcast_ref(), bin.upcast_ref::<gst::Element>()])?;

        // prepare the pipeline
        pipeline
            .set_state(gst::State::Ready)
            .context("could not prepare gstreamer pipeline")?;

        let bus = pipeline.bus().context("failed to get element bus")?;
        let mut bus_stream = bus.stream();

        // start feeding data into the pipeline
        pipeline
            .set_state(gst::State::Playing)
            .context("could not prepare gstreamer pipeline")?;

        loop {
            select! {
                frame = frame_rx.recv_async() => {
                    if let Ok(frame) = frame {
                        // we got a frame, copy the data into gstreamer
                        let mut buf = gst::Buffer::with_size(frame.data.len())
                            .context("failed to allocate gstreamer framebuffer")?;

                        {
                            let buf = buf
                                .get_mut()
                                .context("failed to write to gstreamer framebuffer")?;

                            if let Err(_) = buf.copy_from_slice(0, &frame.data[..]) {
                                bail!("failed to fill gstreamer framebuffer");
                            }

                            // set presentation time of the frame according to
                            // the frame time
                            let frame_time = frame.timestamp - start_time;
                            buf.set_pts(gst::ClockTime::from_mseconds(
                                frame_time.num_milliseconds() as u64
                            ));
                        }

                        appsrc.push_buffer(buf).context("failed to push buffer to appsrc")?;

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

        // stream has ended
        appsrc.end_of_stream().context("ending stream failed")?;
        pipeline
            .set_state(gst::State::Null)
            .context("error while stopping pipeline")?;
        sleep(Duration::from_millis(500)).await;

        Ok(())
    }
}
