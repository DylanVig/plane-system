fn parse_geopoint(env: &str) -> Result<geo::Point, std::io::Error> {
    if let Some((lat, lon)) = env.split_once(',') {
        let lat_float = lat.parse::<f32>().unwrap();
        let lon_float = lon.parse::<f32>().unwrap();
        let Point = geo::Point::new(lon_float, lat_float);
    } else {
        Ok((None, std::io::Error::raw_os_error(&self))) //make invalid command error
    }
    Ok((Point, None));
}

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::bail;
use clap::Subcommand;
use serde::Serialize;

use super::{interface::OperatingMode, state::*};

#[derive(Subcommand, Debug, Clone)]
pub enum ModeRequest {
    /// plane system modes inactive
    Inactive(),

    /// starts state which handles switching between capture and standby, initially starts on standby
    #[clap(subcommand)]
    Search(SearchRequest),

    /// sets the zoom control with the specific presets
    ZoomControl(Presets),
    /// debugging mode, plane system livestreams, saving different videos for the different modes along with denoting metrics such as when each mode was switches into,
    LivestreamOnly,
}

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
    Manual,
}
