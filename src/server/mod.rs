use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tide::{self, Request};

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

    let state = ServerState {};

    let mut app = tide::with_state(state);

    app.at("/").get(|_| async {
        let response = tide::Response::builder(200)
            .body(tide::Body::empty())
            .content_type("text/plain")
            .build();
        Ok(response)
    });

    app.at("/connect")
        .get(|req: Request<ServerState>| async move {
            todo!();
            Ok(tide::Response::new(200))
        });

    app.at("/disconnect")
        .get(|req: Request<ServerState>| async move {
            todo!();
            Ok(tide::Response::new(200))
        });

    let mut api = app.at("/api");

    api.at("/add-roi")
        .post(|mut req: Request<ServerState>| async move {
            let body: AddROIs = req.body_json().await?;

            debug!("received ROIs: {:?}", &body);

            Ok(tide::Response::new(200))
        });

    let address = "127.0.0.1:8080";
    info!("initialized server");
    info!("listening at {}", address);

    app.listen(address).await?;
    Ok(())
}
