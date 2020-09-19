use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct RegionOfInterestId(usize);

static LAST_ROI_ID: AtomicUsize = AtomicUsize::new(0);

impl RegionOfInterestId {
    pub fn new() -> Self {
        let id = LAST_ROI_ID.fetch_add(1, Ordering::SeqCst);
        RegionOfInterestId(id)
    }
}

#[derive(Clone, Debug)]
pub struct RegionOfInterest {
    location: GpsLocation,
    times_captured: u32,
    id: RegionOfInterestId,
}

impl RegionOfInterest {
    pub fn with_location(location: GpsLocation) -> Self {
        RegionOfInterest {
            location,
            times_captured: 0,
            id: RegionOfInterestId::new(),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Mode {
    Idle,
    Fixed,
    Tracking,
    OffAxis, // TODO figure out logic for off-axis targets
}

#[derive(Debug, Default, Clone, Copy)]
pub struct GpsLocation {
    latitude: f32,
    longitude: f32,
}

impl GpsLocation {
    pub fn new(latitude: f32, longitude: f32) -> Self {
        GpsLocation {
            latitude,
            longitude,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Image {
    path: PathBuf,
    mode: Mode,
    geotag: GpsLocation,
}
