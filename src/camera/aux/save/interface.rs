use gst::prelude::*;

pub struct SaveInterface {
  pipeline: Option<gst::Element>,
  address: String,
  cameras: Vec<String>,
}

impl SaveInterface {
  pub fn new(
    mode: bool,
    address: String,
    rpi_cameras: Vec<String>,
    test_cameras: Vec<String>,
  ) -> anyhow::Result<Self> {
    // Initialize GStreamer
    gst::init().unwrap();

    let cameras = match mode {
      false => test_cameras,
      true => rpi_cameras,
    };

    let pipeline = None;
    Ok(Self {
      pipeline,
      address,
      cameras,
    })
  }
  pub fn start_save(&mut self) -> anyhow::Result<()> {
    info!("Starting saver");

    let mut command = String::from("");

    for i in 0..self.cameras.len() {
      let new_command = &format!(
        "{} ! queue ! x264enc ! mpegtsmux ! filesink location={}{}.mp4",
        &self.cameras[i],
        &self.address,
        i.to_string()
      );
      command = format!("{}\n{}", command, new_command)
    }

    self.pipeline = Some(gst::parse_launch(&command).unwrap());

    // Start playing
    self
      .pipeline
      .as_ref()
      .unwrap()
      .set_state(gst::State::Playing)
      .expect("Unable to set the pipeline to the `Playing` state");

    Ok(())
  }

  pub fn end_save(&mut self) -> anyhow::Result<()> {
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
