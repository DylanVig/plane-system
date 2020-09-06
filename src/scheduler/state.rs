use std::{path::PathBuf, sync::atomic::{AtomicUsize, Ordering}};

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
    latitude: f64,
    longitude: f64,
    id: RegionOfInterestId,
}

impl RegionOfInterest {
    pub fn new() -> Self {
        Self::with_coords(0., 0.)
    }

    pub fn with_coords(latitude: f64, longitude: f64) -> Self {
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
    geotag: (),
}
