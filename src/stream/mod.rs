use gst::prelude::*;

pub fn run() {
    // Initialize GStreamer
    gst::init().unwrap();

    //autovideosrc ! videoconvert ! autovideosink

    // Create the GStreamer pipeline
    let pipeline = gst::parse_launch(&format!(
        "autovideosrc !  videoscale \
    ! videoconvert ! x264enc tune=zerolatency bitrate=500 \
    speed-preset=superfast ! rtph264pay ! \
    udpsink host=127.0.0.1 port=5000",
    ))
    .unwrap();

    // Start playing
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    // Wait until error or EOS
    let bus = pipeline.get_bus().unwrap();
    for _ in bus.iter_timed(gst::CLOCK_TIME_NONE) {}

    // Shutdown pipeline
    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
}
