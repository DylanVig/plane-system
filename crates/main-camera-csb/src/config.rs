use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CsbConfig {
    pub gpio_int: u8,
    pub gpio_ack: u8,
    pub i2c: Option<u8>,
}
