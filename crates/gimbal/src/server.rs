use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use ps_client::CommandSender;
use serde::Deserialize;
use tokio::sync::oneshot;
use tracing::debug;

use crate::command::*;

#[derive(Clone, Debug)]
struct ServerState {
    cmd_tx: CommandSender<GimbalRequest, GimbalResponse>,
}

pub async fn serve(
    cmd_tx: CommandSender<GimbalRequest, GimbalResponse>,
) -> Result<(), anyhow::Error> {
    use axum::routing::*;

    let app = axum::Router::new()
        .route("/set-focal-length", post(control_gimbal))
        .with_state(ServerState { cmd_tx });

    axum::Server::bind(&"192.168.1.24".parse().unwrap())
        .serve(app.into_make_service())
        .await?;

    Ok(())
    
}

// endpoint sends a distance search request to the plane system
async fn control_gimbal(
    State(state): State<ServerState>,
    request: Json<GimbalRequestJSON>,
) -> Response {
    debug!("hit gimbal control endpoint");
    let req = GimbalRequest::Control {
        pitch: request.pitch,
        roll: request.roll,
    };
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

#[derive(Deserialize)]
struct GimbalRequestJSON {
    pitch: f64,
    roll: f64,
} 
