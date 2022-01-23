use std::net::SocketAddr;


use gst::prelude::*;

pub struct StreamInterface {
    pipeline: Option<gst::Element>,
    address: SocketAddr,
    cameras: Vec<String>,
}

impl StreamInterface {
    pub fn new(address: SocketAddr, cameras: Vec<String>) -> anyhow::Result<Self> {
        // Initialize GStreamer
        gst::init().unwrap();

        Ok(Self {
            pipeline: None,
            address,
            cameras,
        })
    }
    pub fn start_stream(&mut self) -> anyhow::Result<()> {
        info!("starting stream");
        let mut command = String::from("");

        for i in 0..self.cameras.len() {
            let part = format!("{} ! videoconvert ! x264enc tune=zerolatency bitrate=500 speed-preset=superfast ! rtph264pay ! udpsink host={} port={} ", &self.cameras[i], self.address.ip(), self.address.port() + i as u16);
            command += &part;
        }

        self.pipeline = Some(gst::parse_launch(&command).unwrap());

        // Start playing
        self.pipeline
            .as_ref()
            .unwrap()
            .set_state(gst::State::Playing)
            .expect("Unable to set the pipeline to the `Playing` state");

        Ok(())
    }

    pub fn end_stream(&mut self) -> anyhow::Result<()> {
        // End pipeline
        self.pipeline
            .as_ref()
            .unwrap()
            .set_state(gst::State::Null)
            .expect("Unable to set the pipeline to the `Null` state");
        Ok(())
    }
}
