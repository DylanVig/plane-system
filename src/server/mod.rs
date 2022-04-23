use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use warp::{self, Filter};

use crate::camera::main::CameraCommandGetRequest;
use crate::state::RegionOfInterest;

use crate::{camera::main::CameraCommandRequest, Channels, Command};

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

    //returns the camera data in a json
    let route_camera = warp::path!("api" / "camera").and(warp::get()).and_then({
        let camera_cmd = channels.camera_cmd.clone();
        move || {
            let camera_cmd = camera_cmd.clone();
            async move {
                let (cmd, chan) = Command::new(CameraCommandRequest::Status);

                camera_cmd.send(cmd);

                let response = chan.await.unwrap().unwrap();

                Ok::<_, Infallible>(warp::reply::json(&response))
            }
        }
    });

    let api = route_online
        .or(route_roi)
        .or(route_telem)
        .or(route_telem_stream)
        .or(route_camera);

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
