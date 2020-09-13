use smol::channel::{Receiver, Sender};

use crate::{interface::camera::CameraError};

use super::Channels;

#[derive(Clone, Debug)]
pub enum CameraCommand {
    TakeImage,
    ZoomIn,
    ZoomOut,
}

#[derive(Clone, Debug)]
pub struct CameraClient {
    pub(crate) channels: Channels<CameraCommand, Result<(), CameraError>>,
}

impl CameraClient {}
