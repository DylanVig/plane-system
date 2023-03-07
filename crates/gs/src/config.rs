// Need to add URL of ground server as config information

use serde::Deserialize;

#[derive(Debug, Deserialize)]

pub struct GsConfig {
    pub address: String,
    pub proxy: Option<String>,
}
