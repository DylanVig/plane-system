use crate::state::GPSLocation;
use smol::channel::{Receiver, Sender};
pub enum GimbalCommand {
    Fixed,
    Tracking(GPSLocation),
}
