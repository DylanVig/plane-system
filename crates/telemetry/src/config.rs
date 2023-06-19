use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct TelemetryConfig {
    /// The number of seconds of telemetry data to retain for each image
    pub retention_period: f32,
}
