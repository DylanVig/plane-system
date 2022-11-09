use std::collections::HashMap;

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use futures::stream::{SelectAll, StreamExt};
use futures::FutureExt;
use gst::glib::clone::Downgrade;
use gst::prelude::*;
use log::*;
use ps_client::{ChannelCommandSink, ChannelCommandSource, Task};
use tokio::select;
use tokio_stream::StreamMap;
use tokio_util::sync::CancellationToken;

use super::*;

pub struct CustomTask {
    save_path: PathBuf,
    /// A map from pipeline names to GStreamer pipeline descriptions.
    pipeline_descs: HashMap<String, String>,
    cmd_tx: ChannelCommandSink<LivestreamRequest, LivestreamResponse>,
    cmd_rx: ChannelCommandSource<LivestreamRequest, LivestreamResponse>,
}

pub fn create_task(config: CustomConfig) -> anyhow::Result<CustomTask> {
    let (cmd_tx, cmd_rx) = flume::bounded(256);

    let mut save_path = config.save_path;
    save_path.push(chrono::Local::now().format("%FT%H-%M-%S").to_string());

    let mut fmt_vars = HashMap::new();
    fmt_vars.insert("save_path".to_owned(), save_path.display().to_string());

    Ok(CustomTask {
        save_path,
        pipeline_descs: config
            .pipelines
            .into_iter()
            .map(|(key, val)| {
                let val = val.join("\n");
                let val = strfmt::strfmt(&val, &fmt_vars)?;

                anyhow::Result::Ok((key, val))
            })
            .collect::<anyhow::Result<HashMap<String, String>>>()
            .context("invalid pipeline format string")?,
        cmd_rx,
        cmd_tx,
    })
}

impl CustomTask {
    pub fn cmd(&self) -> ChannelCommandSink<LivestreamRequest, LivestreamResponse> {
        self.cmd_tx.clone()
    }
}

#[async_trait]
impl Task for CustomTask {
    fn name(&self) -> &'static str {
        "livestream/custom"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            save_path,
            cmd_rx,
            pipeline_descs,
            ..
        } = *self;

        gst::init().context("failed to init gstreamer")?;

        if !save_path.exists() {
            tokio::fs::create_dir_all(&save_path)
                .await
                .context("failed to create save directory")?;
        }

        let mut active_pipelines = HashMap::new();

        let loop_fut = async {
            while let Ok((cmd, ret_tx)) = cmd_rx.recv_async().await {
                let result = 'cmd: {
                    match cmd {
                        LivestreamRequest::Start { name } => {
                            if active_pipelines.contains_key(&name) {
                                break 'cmd Err(anyhow!("pipeline '{name}' is already running"));
                            }

                            let pipeline_desc = match pipeline_descs.get(&name) {
                                Some(pd) => pd,
                                None => {
                                    break 'cmd Err(anyhow!(
                                        "no pipeline named '{name}' is configured"
                                    ))
                                }
                            };

                            debug!("initializing pipeline with description {pipeline_desc}");

                            let pipeline = match init_pipeline(&name, &pipeline_desc) {
                                Ok(ret) => ret,
                                Err(err) => break 'cmd Err(err),
                            };

                            active_pipelines.insert(name, pipeline);

                            Ok(())
                        }
                        LivestreamRequest::Stop { name } => {
                            if let Some(pipeline) = active_pipelines.remove(&name) {

                                debug!("sending eos to pipeline {name}");
                                let bus: gst::Bus =
                                    pipeline.bus().context("pipeline has no bus")?;
                                pipeline.send_event(gst::event::Eos::new());

                                // wait for eos message to appear on the bus
                                // before setting state to null
                                let mut bus_stream = bus.stream_filtered(&[gst::MessageType::Eos]);
                                bus_stream.next().await;
                                debug!("got eos from pipeline {name}");

                                Ok(())
                            } else {
                                Err(anyhow!("pipeline '{name}' is not running"))
                            }
                        }
                    }
                };

                ret_tx.send(result).unwrap();
            }

            Ok(())
        };

        let res: anyhow::Result<()> = select! {
            res = loop_fut => res,
            _ = cancel.cancelled() => Ok(())
        };

        debug!("stopping all running pipelines");

        futures::future::join_all(active_pipelines.into_iter().map(
            |(name, pipeline)| async move {
                match pipeline.bus() {
                    Some(bus) => {
                        debug!("sending eos to pipeline {name}");
                        pipeline.send_event(gst::event::Eos::new());

                        // wait for eos message to appear on the bus
                        // before setting state to null
                        let mut bus_stream = bus.stream_filtered(&[gst::MessageType::Eos]);
                        bus_stream.next().await;
                        debug!("got eos from pipeline {name}");
                    }
                    None => warn!("pipeline {name} has no bus"),
                };
            },
        ))
        .await;

        res
    }
}

fn init_pipeline(name: &str, pipeline_desc: &str) -> anyhow::Result<gst::Element> {
    let pipeline = gst::parse_launch(pipeline_desc).context("failed to initialize the pipeline")?;

    // Prepare
    pipeline
        .set_state(gst::State::Ready)
        .context("failed to prepare the pipeline")?;

    // Start playing
    pipeline
        .set_state(gst::State::Playing)
        .context("failed to start the pipeline")?;

    let bus = pipeline.bus().context("pipeline has no bus")?;
    let name = name.to_owned();

    tokio::spawn(async move {
        use gst::MessageView;

        let mut stream = bus.stream();
        while let Some(msg) = stream.next().await {
            match msg.view() {
                MessageView::Eos(eos) => {
                    info!("end of stream '{name}': {eos:?}");
                    break;
                }
                MessageView::Error(err) => {
                    error!("error in stream '{name}': {err:?}");
                    break;
                }
                msg => trace!("stream '{name}': {msg:?}"),
            }
        }

        trace!("exit bus listener");
    });

    Ok(pipeline)
}
