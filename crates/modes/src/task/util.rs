//utility file to abstract out repetitive tasks

//include code for when getting within certain distance

use ps_telemetry::PixhawkTelemetry;

fn transition_by_distance(
    waypoint: Vec<geo::Point>,
    telemetry_rx: watch::Receiver<PixhawkTelemetry>,
    distance_threshold: u64,
    enter: bool,
) {
    let buffer = if enter {1} else {-1}; //"grace period" for cc command action
    let distance = distance_threshold + buffer; 
    let wait_to_check = 250;
    while(in_range(waypoint, telemetry_rx, distance_threshold)!=enter) {
        sleep(Duration::from_milis(wait_to_check)).await; 
    }
    OK()
}


//include code for grabbing plane distance
fn get_telemetry(telemetry_rx: watch::Receiver<PixhawkTelemetry>) {
    let telemetry = telemetry_rx.borrow(); 
    lon_float = telemetry.pixhawk.as_ref().map(|p| p.position.0.point.x());
    lat_float = telemetry.pixhawk.as_ref().map(|p| p.position.0.point.y());
    geo::Point::new(lon_float, lat_float)
}


fn in_range (waypoint: Vec<geo::Point>,
    telemetry_rx: watch::Receiver<PixhawkTelemetry>,
    distance_threshold: u64,) {
        let distance = 0;
        let telemetry_point = get_telemetry(telemetry);
        for point in waypoint {
            distance += point.euclidean_distance() / waypoint.len();
        }
       distance <= distance_threshold

    }


async fn start_cc(main_camera_tx: flume::Sender<CameraRequest>) {
    main_camera_tx.send(CameraRequest::ContinuousCapture(
        ps_main_camera::CameraContinuousCaptureRequest::Start,
    ));
}

async fn end_cc(main_camera_tx: flume::Sender<CameraRequest>) {
    main_camera_tx.send(CameraRequest::ContinuousCapture(
        ps_main_camera::CameraContinuousCaptureRequest::Stop,
    ));
}   
