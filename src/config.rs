use clap::Clap;

#[derive(Debug, Deserialize)]
pub struct PixhawkConfig {
    port: u16,
}

#[derive(Debug, Deserialize)]
pub struct PlaneSystemConfig {
    pixhawk: PixhawkConfig,
}

impl PlaneSystemConfig {
    pub fn read() -> Result<Self, ConfigError> {
        let mut c = Config::new();

        c.merge(File::with_name("plane-system"));

        c.try_into()
    }
}
