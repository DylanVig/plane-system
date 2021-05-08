use gst::prelude::*;

pub struct StreamInterface {
  pipeline: Option<gst::Element>,
  mode: bool,
  address: String,
}

impl StreamInterface {
  pub fn new(mode: bool, address: String) -> anyhow::Result<Self> {
    // Initialize GStreamer
    gst::init().unwrap();

    let pipeline = None;
    Ok(Self {
      pipeline,
      mode,
      address,
    })
  }
  pub fn start_stream(&mut self) -> anyhow::Result<()> {
    info!("Starting stream");
    let ip = &self.address;

    match self.mode {
      false => self.pipeline = Some(gst::parse_launch(&format!(
      "videotestsrc pattern=ball ! videoconvert ! x264enc tune=zerolatency bitrate=500 speed-preset=superfast ! rtph264pay ! udpsink host={} port=5000 \
      videotestsrc ! videoconvert ! x264enc tune=zerolatency bitrate=500 speed-preset=superfast ! rtph264pay ! udpsink host={} port=5001 \
      autovideosrc ! videoconvert ! x264enc tune=zerolatency bitrate=500 speed-preset=superfast ! rtph264pay ! udpsink host={} port=5002", ip, ip, ip
      // "autovideosrc ! videoconvert ! x264enc tune=zerolatency bitrate=500 speed-preset=superfast ! rtph264pay ! udpsink clients={}:5000,{}:5001", ip, ip
    // 
    ))
    .unwrap()),
    true => self.pipeline = Some(gst::parse_launch(&format!("rpicamsrc ! h264parse ! x264enc ! rtph264pay config-interval=1 pt=96 ! gdppay ! udpsink clients={}:5000,{}:5001", ip, ip))
    .unwrap())
    };

    // Start playing
    self
      .pipeline
      .as_ref()
      .unwrap()
      .set_state(gst::State::Playing)
      .expect("Unable to set the pipeline to the `Playing` state");

    Ok(())
  }

  pub fn end_stream(&mut self) -> anyhow::Result<()> {
    // End pipeline
    self
      .pipeline
      .as_ref()
      .unwrap()
      .set_state(gst::State::Null)
      .expect("Unable to set the pipeline to the `Null` state");
    Ok(())
  }
}
