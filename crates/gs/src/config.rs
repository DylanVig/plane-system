// Need to add URL of ground server as config information

use serde::Deserialize;

#[derive(Debug, Deserialize)]

//Use URL type later
pub struct GsConfig {
    pub address: String,
}
