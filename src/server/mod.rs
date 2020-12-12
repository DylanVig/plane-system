use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::stream::{Stream, StreamExt};
use warp::{self, Filter};

use crate::state::{RegionOfInterest, TelemetryInfo};
use crate::Channels;

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
    use tokio_compat_02::FutureExt;

    info!("initializing server");

    let telemetry_receiver = Arc::new(channels.telemetry.clone());

    let route_roi = warp::path!("api" / "roi")
        .and(warp::post())
        .and(warp::body::json())
        .map(move |body: AddROIs| {
            debug!("received ROIs: {:?}", &body);
            warp::reply()
        });

    let route_telem = warp::path!("api" / "telemetry" / "now")
        .and(warp::get())
        .and_then({
            let telemetry = telemetry_receiver.clone().borrow().clone();
            move || async move { Ok::<_, Infallible>(warp::reply::json(&telemetry)) }
        });

    let route_telem_stream = warp::path!("api" / "telemetry" / "stream")
        .and(warp::get())
        .map({
            let telemetry_receiver = channels.telemetry.clone();

            move || {
                let telemetry_stream = futures::stream::unfold(
                    telemetry_receiver.clone(),
                    |mut telemetry_receiver| async move {
                        let res = telemetry_receiver.changed().await;
            
                        let telemetry = telemetry_receiver.borrow().clone();
            
                        Some((res.map(|_| telemetry), telemetry_receiver))
                    },
                );
                
                warp::sse::reply(
                    telemetry_stream
                        .map(|res| res.map(|telemetry| warp::sse::json(telemetry))),
                )
            }
        });

    let api = route_roi.or(route_telem).or(route_telem_stream);

    info!("initialized server");

    async {
        let (_, server) = warp::serve(api).bind_with_graceful_shutdown(address, async move {
            channels
                .interrupt
                .subscribe()
                .recv()
                .await
                .expect("error while waiting on interrupt channel");

            debug!("server recv interrupt");
        });

        info!("listening at {:?}", address);

        server.await;
    }
    .compat()
    .await;

    Ok(())
}
