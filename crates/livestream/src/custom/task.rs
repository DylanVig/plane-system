use std::collections::HashMap;

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use futures::stream::{SelectAll, StreamExt};
use gst::traits::ElementExt;
use log::*;
use ps_client::{ChannelCommandSink, ChannelCommandSource, Task};
use tokio::select;
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

        if !save_path.exists() {
            tokio::fs::create_dir_all(&save_path)
                .await
                .context("failed to create save directory")?;
        }

        let mut active_pipelines = HashMap::new();
        let mut active_bus_streams = SelectAll::<gst::bus::BusStream>::new();

        loop {
            select! {
                _ = cancel.cancelled() => {
                    break;
                }
                msg = active_bus_streams.next() => {
                    use gst::MessageView;

                    if let Some(msg) = msg {
                        match msg.view() {
                            MessageView::Eos(eos) => {
                                info!("end of stream: {eos:?}")
                            }
                            MessageView::Error(err) => {
                                error!("error in stream: {err:?}")
                            }
                            msg => trace!("stream: {msg:?}")
                        }
                    }
                }
                cmd = cmd_rx.recv_async() => {
                    if let Ok((cmd, ret_tx)) = cmd_rx.recv_async().await {
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

                                    let (pipeline, bus) = match init_pipeline(&pipeline_desc) {
                                        Ok(ret) => ret,
                                        Err(err) => break 'cmd Err(err),
                                    };

                                    active_pipelines.insert(name, pipeline);
                                    active_bus_streams.push(bus.stream());

                                    Ok(())
                                }
                                LivestreamRequest::Stop { name } => {
                                    if let Some(pipeline) = active_pipelines.remove(&name) {
                                        if let Err(err) = pipeline.set_state(gst::State::Null) {
                                            break 'cmd Err(err).context("failed to stop pipeline");
                                        }

                                        Ok(())
                                    } else {
                                        Err(anyhow!("pipeline '{name}' is already running"))
                                    }
                                }
                            }
                        };

                        let _ = ret_tx.send(result);
                    }
                }
            }
        }

        debug!("stopping all running pipelines");

        for (name, pipeline) in active_pipelines {
            if let Err(err) = pipeline.set_state(gst::State::Null) {
                error!("failed to stop pipeline {name}: {err:?}");
            }
        }

        Ok(())
    }
}

fn init_pipeline(pipeline_desc: &str) -> anyhow::Result<(gst::Element, gst::Bus)> {
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

    Ok((pipeline, bus))
}
