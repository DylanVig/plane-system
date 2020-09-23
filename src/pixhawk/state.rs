#[derive(Debug)]
pub struct PixhawkTelemetry {
    gps: Option<PixhawkTelemetryCoords>,
    attitude: Option<PixhawkTelemetryAttitude>,
    geotag: Option<PixhawkTelemetryCoords>,
}

#[derive(Debug)]
pub struct PixhawkTelemetryCoords {
    latitude: f32,
    longitude: f32,
    altitude: f32,
}

#[derive(Debug)]
pub struct PixhawkTelemetryAttitude {
    roll: f32,
    pitch: f32,
    yaw: f32,
}
