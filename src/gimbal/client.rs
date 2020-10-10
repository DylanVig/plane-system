use anyhow;

use crate::{
    Channels,
}

pub struct GimbalClient {
}

impl GimbalClient {
    pub async fn connect(channels: Channels) -> anyhow::Result<Self> {
        Ok(())
    }
}