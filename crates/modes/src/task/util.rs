//utility file to abstract out repetitive tasks

use chrono::ParseError;
//include code for when getting within certain distance
use geo::EuclideanDistance;
use ps_main_camera::CameraRequest;
//use ps_telemetry::PixhawkTelemetry;
use ps_telemetry::Telemetry;
use tokio::time::sleep;
use tokio::sync::watch;
use tokio::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseTelemetryError {
    #[error("invalid latitude given")]
    InvalidLat,
    #[error("missing longitude given")]
    InvalidLon,
}

pub async fn transition_by_distance(
    waypoint: &Vec<geo::Point>,
    telemetry_rx: &watch::Receiver<Telemetry>,
    distance_threshold: u64,
    enter: bool,
) {
    //"grace period" for cc command action
    let distance = if enter {distance_threshold + 1} else {distance_threshold - 1};
    let wait_to_check = 250;
    while in_range(waypoint, telemetry_rx, distance_threshold) != enter {
        sleep(Duration::from_millis(wait_to_check)).await;
    }
}

//include code for grabbing plane distance
fn get_telemetry(telemetry_rx: &watch::Receiver<Telemetry>) -> Result<geo::Point, ParseTelemetryError> { //should maybe return a result
    let telemetry = telemetry_rx.borrow();
    let mut lon_float: f64 = 0.0;
    let mut lat_float: f64 = 0.0;
    let lon_float_opt = telemetry.pixhawk.as_ref().map(|p| p.position.0.point.x());
    match lon_float_opt {
        Some(lon) => {lon_float = lon as f64;}
        None => return return Err(ParseTelemetryError::InvalidLon)
    }
    let lat_float_opt = telemetry.pixhawk.as_ref().map(|p| p.position.0.point.y());
    match lat_float_opt {
        Some(lat) => {lat_float = lat as f64;}
        None => return Err(ParseTelemetryError::InvalidLat)
    }
    Ok(geo::Point::new(lon_float, lat_float))
}

fn in_range(
    waypoint: &Vec<geo::Point>,
    telemetry_rx: &watch::Receiver<Telemetry>, //should maybe all be just Tele?
    distance_threshold: u64,
) -> bool {
    let distance = 0.0;
    let telemetry_point = get_telemetry(&telemetry_rx);
    match telemetry_point {
        Ok(_) => {
            for point in waypoint {
                distance += point.euclidean_distance(&telemetry_point) / (waypoint.len() as f64);
            }
            distance as u64 <= distance_threshold
        }
        Err(_) => false, //is this losing information by doing this?
    }

   
}

pub async fn start_cc(main_camera_tx: &flume::Sender<CameraRequest>) {
    main_camera_tx.send(CameraRequest::ContinuousCapture(
        ps_main_camera::CameraContinuousCaptureRequest::Start,
    ));
}

pub async fn end_cc(main_camera_tx: &flume::Sender<CameraRequest>) {
    main_camera_tx.send(CameraRequest::ContinuousCapture(
        ps_main_camera::CameraContinuousCaptureRequest::Stop,
    ));
}
