use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct MainCameraConfig {
    #[cfg(feature = "csb")]
    pub current_sensing: Option<ps_main_camera_csb::CurrentSensingConfig>,
}
