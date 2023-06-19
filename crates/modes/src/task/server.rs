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
        .with_state(ServerState { cmd_tx });

    axum::Server::bind(&"0.0.0.0:4200".parse().unwrap())
        .serve(app.into_make_service())
        .await?;

    Ok(())
}

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
