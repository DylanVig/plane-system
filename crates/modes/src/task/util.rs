//utility file to abstract out repetitive tasks

//include code for when getting within certain distance

fn trigger_distance(
    waypoint: Vec<geo::Point>,
    telemetry: Option<Telemetry>,
    distance_threshold: u64,
) {
    let distance = distance_threshold + 1;
    while (distance > distance_threshold) {
        sleep(Duration::from_millis(250)).await;
        let distance = 0;
        let telemetry_point = get_telemetry(telemetry);
        for point in waypoint {
            distance += point.euclidean_distance() / waypoint.len();
        }
    }
    OK()
}
//include code for grabbing plane distance

fn get_telemetry(telemetry: Option<Telemetry>) {
    lon_float = telemetry.pixhawk.as_ref().map(|p| p.position.0.point.x());
    lat_float = telemetry.pixhawk.as_ref().map(|p| p.position.0.point.y());
    geo::Point::new(lon_float, lat_float)
}
