use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::sync::oneshot;
use warp::{self, Filter};

use crate::scheduler::{Roi, SchedulerCommand};
use crate::Channels;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct AddROIs {
    pub rois: Vec<Roi>,
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
        .then({
            let channels = channels.clone();
            move |body: AddROIs| {
                let channels = channels.clone();
                async move {
                    debug!("received ROIs: {:?}", &body);

                    let (tx, rx) = oneshot::channel();

                    channels
                        .scheduler_cmd
                        .send(SchedulerCommand::AddROIs {
                            rois: body.rois,
                            tx,
                        })
                        .unwrap();

                    rx.await.unwrap();

                    warp::reply()
                }
            }
        });

    let roi_query = warp::path!("api" / "rois").and(warp::get()).then({
        let channels = channels.clone();
        move || {
            let channels = channels.clone();
            async move {
                debug!("received request for list of ROIs");

                let (tx, rx) = oneshot::channel();
                channels
                    .scheduler_cmd
                    .send(SchedulerCommand::GetROIs { tx })
                    .unwrap();

                let rois = rx.await.unwrap();

                debug!("received list of ROIs {:?}", rois);

                warp::reply::json(&rois)
            }
        }
    });

    let captures_query = warp::path!("api" / "captures").and(warp::get()).then({
        let channels = channels.clone();
        move || {
            let channels = channels.clone();
            async move {
                debug!("Recieved request for captures.");
                let (tx, rx) = oneshot::channel();
                channels
                    .scheduler_cmd
                    .send(SchedulerCommand::GetCaptures { tx })
                    .unwrap();

                let captures = rx.await.unwrap();
                debug!("recieved captures {:?}", captures);

                warp::reply::json(&captures)
            }
        }
    });

    let cap_plan_query = warp::path!("api" / "target").and(warp::get()).then({
        let channels = channels.clone();
        move || {
            let channels = channels.clone();
            async move {
                debug!("Recieved request for target ROI");
                let (tx, rx) = oneshot::channel();
                channels
                    .scheduler_cmd
                    .send(SchedulerCommand::GetTargetROI { tx })
                    .unwrap();

                let target = rx.await.unwrap();
                debug!("recieved target roi {:?}", target);

                warp::reply::json(&target)
            }
        }
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

    let api = route_online
        .or(route_roi)
        .or(route_telem)
        .or(route_telem_stream)
        .or(roi_query);

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
