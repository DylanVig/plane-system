use crate::state::{
    RegionOfInterest,
    Image,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, Eq, PartialEq, Debug, Serialize, Deserialize)]
pub struct CaptureRequestId(usize);

static LAST_REQUEST_ID: AtomicUsize = AtomicUsize::new(0);

impl CaptureRequestId {
    pub fn new() -> Self {
        let id = LAST_REQUEST_ID.fetch_add(1, Ordering::SeqCst);
        CaptureRequestId(id)
    }
}

#[derive(Copy, Clone, Debug)]
pub enum CaptureType {
    Fixed,
    Tracking(RegionOfInterest),
}

#[derive(Copy, Clone, Debug)]
pub struct CaptureRequest {
    pub request_id: CaptureRequestId,
    pub capture_type: CaptureType,
}

impl CaptureRequest {
    pub fn from_capture_type(capture_type: CaptureType) -> Self {
        Self {
            request_id: CaptureRequestId::new(),
            capture_type,
        }
    }
}

pub struct CaptureResponse {
    request_id: usize,
}