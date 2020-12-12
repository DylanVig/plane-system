use num_traits::FromPrimitive;
use simplebgc::*;
use std::io::{Read, Write};
use std::time::Duration;

use super::GimbalInterface;

pub struct SoftwareGimbalInterface {}

impl SoftwareGimbalInterface {}

impl GimbalInterface for SoftwareGimbalInterface {
    fn new() -> anyhow::Result<Self> {
        unimplemented!()
    }

    fn control_angles(&mut self, mut roll: f64, mut pitch: f64) -> anyhow::Result<()> {
        unimplemented!()
    }
}
