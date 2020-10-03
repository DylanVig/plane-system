use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use warp::{self, Filter};

use crate::state::RegionOfInterest;

#[derive(Clone)]
struct ServerState {}

enum ServerMessage {
    AddROIs(Vec<RegionOfInterest>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AddROIs {
    pub rois: Vec<RegionOfInterest>,
    pub client_type: ClientType,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ClientType {
    MDLC,
    ADLC,
}

pub async fn serve() -> Result<(), std::io::Error> {
    info!("initializing server");

    let add_roi = warp::path!("api" / "add-roi")
        .and(warp::post())
        .and(warp::body::json())
        .map(move |body: AddROIs| {
            debug!("received ROIs: {:?}", &body);
            warp::reply()
        });

    let address = ([127, 0, 0, 1], 8080);
    info!("initialized server");
    info!("listening at {:?}", address);

    warp::serve(add_roi).run(address).await;

    Ok(())
}
