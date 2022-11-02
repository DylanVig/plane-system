use std::path::{Path, PathBuf};

use anyhow::Context;
use futures::stream::StreamExt;
use gst::prelude::*;
use log::*;

pub struct SaveInterface {
    pipeline: Option<gst::Element>,
    save_path: PathBuf,
    save_ext: String,
    cameras: Vec<String>,
}

impl SaveInterface {
    pub fn new(
        save_path: impl AsRef<Path>,
        save_ext: String,
        cameras: Vec<String>,
    ) -> anyhow::Result<Self> {
        // Initialize GStreamer
        gst::init().context("failed to init gstreamer")?;

        let mut save_path = save_path.as_ref().to_owned();
        save_path.push(chrono::Local::now().format("%FT%H-%M-%S").to_string());

        let pipeline = None;
        Ok(Self {
            pipeline,
            save_path,
            save_ext,
            cameras,
        })
    }

    pub fn start_save(&mut self) -> anyhow::Result<()> {
        if self.pipeline.is_some() {
            info!("saver is already running");
        }

        info!("Starting saver");

        let mut command = String::from("");

        for i in 0..self.cameras.len() {
            let mut path = self.save_path.clone();
            path.push(format!("camera_{i}"));
            path.set_extension(self.save_ext);

            let new_command = &format!("{} ! filesink location={:?}", &self.cameras[i], &path);
            command = format!("{}\n{}", command, new_command)
        }

        info!("running gstreamer pipeline: {command}");

        self.pipeline =
            Some(gst::parse_launch(&command).context("failed to start gstreamer pipeline")?);

        // Start playing
        self.pipeline
            .as_ref()
            .unwrap()
            .set_state(gst::State::Playing)
            .context("failed to set the pipeline to the `Playing` state")?;

        Ok(())
    }

    pub async fn end_save(&mut self) -> anyhow::Result<()> {
        if let Some(pipeline) = &self.pipeline {
            let bus = pipeline.bus().context("pipeline has no bus")?;
            let mut bus_stream = bus.stream_filtered(&[gst::MessageType::Eos]);

            debug!("sending eos to pipeline");
            pipeline.send_event(gst::event::Eos::new());

            debug!("waiting for eos message from bus");
            bus_stream.next().await.context("no message from bus")?;

            debug!("setting pipeline to null state");
            pipeline
                .set_state(gst::State::Null)
                .context("failed to set the pipeline to the `Null` state")?;

            debug!("dropping pipeline");
        }

        self.pipeline = None;

        Ok(())
    }
}
