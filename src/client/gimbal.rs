use smol::channel::{Receiver, Sender};
use crate::state::GPSLocation;
pub enum GimbalCommand {
    Fixed,
    Tracking(GPSLocation),
}

