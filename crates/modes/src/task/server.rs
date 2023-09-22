use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use ps_client::CommandSender;
use tokio::sync::oneshot;
use tracing::debug;

use crate::command::*;

#[derive(Clone, Debug)]
struct ServerState {
    cmd_tx: CommandSender<ModeRequest, ModeResponse, ModeError>,
}

pub async fn serve(
    cmd_tx: CommandSender<ModeRequest, ModeResponse, ModeError>,
) -> Result<(), anyhow::Error> {
    use axum::routing::*;

    let app = axum::Router::new()
        .route("/pan-search", get(run_panning))
        .route("/manual-search", get(run_manual))
        .route("/distance-search", get(run_distance))
        .route("/time-search", get(run_time))
        .with_state(ServerState { cmd_tx });
    // maybe need to pass state to each, let's see..

    axum::Server::bind(&"0.0.0.0:4200".parse().unwrap())
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

// endpoint sends a request to the plane system to start panning
async fn run_panning(State(state): State<ServerState>) -> Response {
    debug!("hit pan search http endpoint");

    let req = ModeRequest::Search(SearchRequest::Panning {});
    let (ret_tx, ret_rx) = oneshot::channel();
    state.cmd_tx.send_async((req, ret_tx)).await;

    let response = match ret_rx.await {
        Ok(response) => response,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    match response {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

// endpoint sends a manual search request to the plane system, stops or starts cc
async fn run_manual(State(state): State<ServerState>) -> Response {
    debug!("hit continous capture http endpoint");

    let req = ModeRequest::Search(SearchRequest::Manual {});
    let (ret_tx, ret_rx) = oneshot::channel();
    state.cmd_tx.send_async((req, ret_tx)).await;

    let response = match ret_rx.await {
        Ok(response) => response,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    match response {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

// endpoint sends a distance search request to the plane system 
async fn run_distance(State(state): State<ServerState>) -> Response {
    debug!("hit distance http endpoint");
    //TODO: get actual waypoints from autopilot to store in ps_modes for this request
    let mut waypoints: Vec<geo::Point> = Vec::new();
    waypoints.push(geo::Point::new(1.123, 1.5));
    let req = ModeRequest::Search(SearchRequest::Distance{distance:u64 = 10, waypoint:Vec<geo::Point>= waypoints});
    let (ret_tx, ret_rx) = oneshot::channel();
    state.cmd_tx.send_async((req, ret_tx)).await;

    let response = match ret_rx.await {
        Ok(response) => response,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    match response {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

// endpoint sends a distance search request to the plane system 
async fn run_time(request: Json<TimeRequest>, State(state): State<ServerState>) -> Response {
    debug!("hit time http endpoint");
    let req = ModeRequest::Search(SearchRequest::Time{active: u64 = request.active, inactive: u64 = request.inactive});
    let (ret_tx, ret_rx) = oneshot::channel();
    state.cmd_tx.send_async((req, ret_tx)).await;

    let response = match ret_rx.await {
        Ok(response) => response,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    match response {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

struct TimeRequest {
    active: u64,
    inactive: u64
}