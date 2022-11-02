use std::path::{Path, PathBuf};

use anyhow::Context;
use gst::prelude::*;
use log::*;

pub struct SaveInterface {
    pipeline: Option<gst::Element>,
    path: PathBuf,
    cameras: Vec<String>,
}

impl SaveInterface {
    pub fn new(path: impl AsRef<Path>, cameras: Vec<String>) -> anyhow::Result<Self> {
        // Initialize GStreamer
        gst::init().context("failed to init gstreamer")?;

        let pipeline = None;
        Ok(Self {
            pipeline,
            path: path.as_ref().to_owned(),
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
            let mut path = self.path.clone();
            path.push(i.to_string());
            path.set_extension("mp4");

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

    pub fn end_save(&mut self) -> anyhow::Result<()> {
        if let Some(pipeline) = &self.pipeline {
            debug!("sending eos to pipeline");
            pipeline.send_event(gst::event::Eos::new());

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
