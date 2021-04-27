use anyhow::Context;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
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
    info!("initializing server");

    let route_online = warp::path!("api" / "online")
        .and(warp::get())
        .map(move || warp::reply::json(&"ok"));

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

                warp::sse::reply(telemetry_stream.map(|res| {
                    res.map(|telemetry| warp::sse::Event::default().json_data(telemetry).unwrap())
                }))
            }
        });

    let route_test = warp::path!("api" / "test").and(warp::get()).map(|| {
        let response = "hi";
        warp::reply::json(&response)
    });

    let route_image = warp::path!("api" / "image").and(warp::fs::file("src/images/2.jpg"));
    //
    /*
     * This is the main thing you will be adding: The new route. You'll need to specificy the route,
     * which basically means specifying the URL that can be used to reach this point of the plane
     * system. You'll then need to speficy what the route does when it receives a request. This is
     * where the warp framework comes in. As I said in the write-up, you'll want to look at the
     * warp::fs::file function. The general format of the route will look somewhat similar to the
     * two routes defined above (route_roi and route_telem), but can be shorter and less complex.
     */

    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec![
            "User-Agent",
            "Sec-Fetch-Mode",
            "Referer",
            "Origin",
            "Access-Control-Request-Method",
            "Access-Control-Request-Headers",
            "content-type",
            "x-requested-with",
        ])
        .allow_methods(vec!["GET", "POST", "DELETE", "PUT"]);

    /*
     * You'll also need to make a change to the line below. It basically bundles all the
     * routes together into one object that can be served up for other pieces of software,
     * like the frontend, to access. The format is exactly the same as how you see route_telem
     * being added. The .with(cors) bit at deals with some annoying shenanigans, but basically
     * allows you to access the plane system from the frontend running locally on your computer.
     */
    let api = route_roi
        .or(route_telem)
        .or(route_test)
        .or(route_image)
        .or(route_roi)
        .or(route_online)
        .or(route_telem_stream)
        .with(cors);
    
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
    .await;

    Ok(())
}
