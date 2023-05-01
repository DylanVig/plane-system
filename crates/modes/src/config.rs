use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ModesConfig {
    pub gimbal_positions : Vec<(f64, f64)>
}