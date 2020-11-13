use crate::state::RegionOfInterest;

pub enum GimbalState {
    Fixed,
    Tracking(RegionOfInterest),
}
