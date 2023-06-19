use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct CsbConfig {
    pub gpio_int: u8,
    pub gpio_ack: u8,
    pub i2c: Option<CsbI2cConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CsbI2cConfig {
    pub bus: u8,
    pub addr: u16,
}
