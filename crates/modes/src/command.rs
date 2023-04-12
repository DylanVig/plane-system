use anyhow::bail;
use clap::Subcommand;
use geo::coords_iter::GeometryCoordsIter::Point;
use geo::coords_iter::GeometryExteriorCoordsIter::Point;
use geo::Geometry::Point;
use serde::Serialize;
use std::{
    collections::HashMap,
    num::ParseFloatError,
    path::{Path, PathBuf},
    str::FromStr,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParsePointError {
    #[error("invalid coordinates given")]
    InvalidCoord(#[from] ParseFloatError),
    #[error("missing comma")]
    MissingComma,
}

fn parse_geopoint(env: &str) -> Result<geo::Point, ParsePointError> {
    if let Some((lat, lon)) = env.split_once(',') {
        let lat_float = lat.parse::<f64>()?;
        let lon_float = lon.parse::<f64>()?;
        return Ok(geo::Point::new(lon_float, lat_float));
    } else {
        return Err(ParsePointError::MissingComma);
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum ModeRequest {
    /// plane system modes inactive
    Inactive,

    /// starts state which handles switching between capture and standby, initially starts on standby
    #[clap(subcommand)]
    Search(SearchRequest),

    /// sets the zoom control with the specific presets
    #[clap(subcommand)]
    ZoomControl(Presets),
    /// debugging mode, plane system livestreams, saving different videos for the different modes along with denoting metrics such as when each mode was switches into,
    LivestreamOnly,
}

#[derive(Debug)]
pub enum Presets {
    None,
    Expresetname1,
    Expresetname2,
    Expresetname3,
    Expresetname4,
    Expresetname5,
}
#[derive(Subcommand, Debug, Clone)]
pub enum SearchRequest {
    //Captures for a given active interval and stays inactive for a given inactive interval
    Time {
        active: u16,   //time measured in seconds
        inactive: u16, //time measured in seconds
    },
    //Activates search when in a given range of a waypoint, deactivates when exiting range
    Distance {
        distance: u64,             //distance measured in meters
        waypoint: Vec<geo::Point>, //coordinates in [lat,lon]
    },
    //Switches between active and inactive cature are handled by the user
    Manual {
        start: bool, //whether to start or end continous capture (cc)
    },
}
