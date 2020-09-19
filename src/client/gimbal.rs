use crate::state::GPSLocation;
use smol::channel::{Receiver, Sender};
pub enum GimbalState {
    Fixed,
    Tracking(GPSLocation),
}
