use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, net::SocketAddr, sync::Arc, time::SystemTime};
use tokio::sync::RwLock;
use warp::{self, Filter};

use crate::Channels;
use crate::{
    pixhawk::state::PixhawkEvent, pixhawk::state::Telemetry, state::RegionOfInterest,
    util::ReceiverExt,
};

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

pub async fn serve(channels: Arc<Channels>, address: SocketAddr) -> anyhow::Result<()> {
    info!("initializing server");

    let telemetry_receiver = Arc::new(channels.telemetry.clone());

    let route_roi = warp::path!("api" / "roi")
        .and(warp::post())
        .and(warp::body::json())
        .map(move |body: AddROIs| {
            debug!("received ROIs: {:?}", &body);
            warp::reply()
        });

    let route_telem = warp::path!("api" / "telemetry").and(warp::get()).and_then({
        move || {
            let telemetry = telemetry_receiver.clone().borrow().clone();
            async move { Result::<_, Infallible>::Ok(warp::reply::json(&telemetry)) }
        }
    });

    let api = route_roi.or(route_telem);

    info!("initialized server");
    info!("listening at {:?}", address);

    let (_, server) = warp::serve(api).bind_with_graceful_shutdown(address, async move {
        debug!("server recv interrupt");
        channels.interrupt.subscribe().recv().await;
    });

    server.await;

    Ok(())
}
