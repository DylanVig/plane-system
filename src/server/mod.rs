use serde::{Deserialize, Serialize};
use std::{convert::Infallible, sync::Arc, time::SystemTime};
use tokio::sync::RwLock;
use warp::{self, Filter};

use crate::Channels;
use crate::{pixhawk::state::PixhawkMessage, pixhawk::state::Telemetry, state::RegionOfInterest};

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

pub async fn serve(channels: Arc<Channels>) -> anyhow::Result<()> {
    info!("initializing server");

    let pixhawk_telem = Arc::new(RwLock::new(<Telemetry as Default>::default()));

    let route_roi = warp::path!("api" / "roi")
        .and(warp::post())
        .and(warp::body::json())
        .map(move |body: AddROIs| {
            debug!("received ROIs: {:?}", &body);
            warp::reply()
        });

    let route_telem = warp::path!("api" / "telemetry").and(warp::get()).and_then({
        let pixhawk_telem = pixhawk_telem.clone();
        move || {
            let pixhawk_telem = pixhawk_telem.clone();
            async move {
                let telem = pixhawk_telem.read().await.clone();
                Result::<_, Infallible>::Ok(warp::reply::json(&telem))
            }
        }
    });

    let api = route_roi.or(route_telem);

    let address = ([127, 0, 0, 1], 8080);
    info!("initialized server");
    info!("listening at {:?}", address);

    let mut interrupt_recv = channels.interrupt.subscribe();
    let (_, server) = warp::serve(api).bind_with_graceful_shutdown(address, async move {
        debug!("server recv interrupt");
        interrupt_recv.recv().await.unwrap();
    });

    let server_task = tokio::spawn(server);

    let mut interrupt_recv = channels.interrupt.subscribe();
    let mut pixhawk_recv = channels.pixhawk.subscribe();
    let channel_task = tokio::spawn(async move {
        loop {
            let pixhawk_msg = pixhawk_recv.recv().await.unwrap();
            let mut pixhawk_telem = pixhawk_telem.write().await;
            
            match pixhawk_msg {
                PixhawkMessage::Gps { coords } => {
                    pixhawk_telem.coords = Some(coords);
                    pixhawk_telem.coords_timestamp = Some(SystemTime::now());
                }
                PixhawkMessage::Orientation { attitude } => {
                    pixhawk_telem.attitude = Some(attitude);
                    pixhawk_telem.attitude_timestamp = Some(SystemTime::now());
                }
                _ => {}
            }

            if let Ok(()) = interrupt_recv.try_recv() {
                debug!("pixhawk recv interrupt");
                break;
            }
        }
    });

    let results = futures::future::join(server_task, channel_task).await;

    results.0?;
    results.1?;

    Ok(())
}
