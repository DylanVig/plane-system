use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ModesConfig {
    pub gimbal_positions: Vec<GimbalPosition>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GimbalPosition {
    pub pitch: f64,
    pub roll: f64,
}
