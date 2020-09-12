use smol::channel::{Receiver, Sender};

use crate::camera::Camera;

pub enum CameraCommand {
    TakeImage,
    ZoomIn,
    ZoomOut,
}

pub enum 

pub struct CameraClient {
    camera: Camera,
    cmd_recv_channel: (Sender<CameraCommand>, Receiver<CameraCommand>),
    msg_send_channel: (Sender<CameraCommand>, Receiver<CameraCommand>),
}

impl CameraClient {}
