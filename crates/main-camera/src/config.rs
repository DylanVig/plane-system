#[derive(Debug, Deserialize)]
pub struct MainCameraConfig {
    pub kind: CameraKind,

    #[cfg(feature = "csb")]
    pub current_sensing: Option<ps_main_camera_csb::CurrentSensingConfig>,
}
