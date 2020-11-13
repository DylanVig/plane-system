use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct RegionOfInterestId(usize);

static LAST_ROI_ID: AtomicUsize = AtomicUsize::new(0);

impl RegionOfInterestId {
    pub fn new() -> Self {
        let id = LAST_ROI_ID.fetch_add(1, Ordering::SeqCst);
        RegionOfInterestId(id)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegionOfInterest {
    id: RegionOfInterestId,
    location: Coords2D,
    kind: RegionOfInterestKind,

    #[serde(default)]
    times_captured: u32,
}

impl RegionOfInterest {
    pub fn with_location_and_kind(location: Coords2D, kind: RegionOfInterestKind) -> Self {
        RegionOfInterest {
            location,
            times_captured: 0,
            id: RegionOfInterestId::new(),
            kind,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegionOfInterestKind {
    Normal,
    OffAxis,
    EmergentTarget,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    Idle,
    Fixed,
    Tracking,
    OffAxis, // TODO figure out logic for off-axis targets
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Coords2D {
    /// Latitude in degrees
    pub latitude: f32,

    /// Longitude in degrees
    pub longitude: f32,
}

impl Coords2D {
    pub fn new(latitude: f32, longitude: f32) -> Self {
        Coords2D {
            latitude,
            longitude,
        }
    }

    pub fn with_altitude(self, altitude: f32) -> Coords3D {
        Coords3D::new(self.latitude, self.longitude, altitude)
    }
}

impl From<Coords3D> for Coords2D {
    fn from(c: Coords3D) -> Self {
        Coords2D::new(c.latitude, c.longitude)
    }
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Coords3D {
    /// Latitude in degrees
    pub latitude: f32,

    /// Longitude in degrees
    pub longitude: f32,

    /// Altitude in meters
    pub altitude: f32,
}

impl Coords3D {
    pub fn new(latitude: f32, longitude: f32, altitude: f32) -> Self {
        Coords3D {
            latitude,
            longitude,
            altitude,
        }
    }
}

#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Attitude {
    /// Roll in degrees
    pub roll: f32,

    /// Pitch in degrees
    pub pitch: f32,

    /// Yaw in degrees
    pub yaw: f32,
}

impl Attitude {
    pub fn new(roll: f32, pitch: f32, yaw: f32) -> Self {
        Attitude { roll, pitch, yaw }
    }
}

#[derive(Default, Debug, Clone, Copy, Serialize)]
pub struct TelemetryInfo {
    pub plane_attitude: Attitude,
    pub gimbal_attitude: Attitude,
    pub position: Coords3D,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    path: PathBuf,
    mode: Mode,
    geotag: Coords2D,
}
