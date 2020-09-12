use std::{path::PathBuf, sync::atomic::{AtomicUsize, Ordering}};
use crate::state::GPSLocation;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct RegionOfInterestId(usize);

impl RegionOfInterestId {
    pub fn new() -> Self {
        let id = LAST_ROI_ID.fetch_add(1, Ordering::SeqCst);
        RegionOfInterestId(id)
    }
}

static LAST_ROI_ID: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug)]
pub struct RegionOfInterest {
    latitude: f32,
    longitude: f32,
    id: RegionOfInterestId,
}

impl RegionOfInterest {
    pub fn new() -> Self {
        Self::with_coords(0., 0.)
    }

    pub fn with_coords(latitude: f32, longitude: f32) -> Self {
        RegionOfInterest {
            latitude,
            longitude,
            id: RegionOfInterestId::new(),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Mode {
    Idle,
    Fixed,
    Tracking,
    OffAxis,
}

#[derive(Debug, Clone)]
pub struct Image {
    path: PathBuf,
    mode: Mode,
    rois: Vec<RegionOfInterest>,
    geotag: GPSLocation,
}
