use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use ps_client::CommandSender;
use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;
use tracing::debug;
use crate::command::*;

#[derive(Clone, Debug)]
struct ServerState {
    cmd_tx: CommandSender<CameraRequest, CameraResponse>,
}

pub async fn serve(
    cmd_tx: CommandSender<CameraRequest, CameraResponse>,
) -> Result<(), anyhow::Error> {
    use axum::routing::*;

    let app = axum::Router::new()
        .route("/set-zoom-focal-length", post(set_focal_length))
        .route("/set-zoom-level", post(set_level))
        .route("/capture", get(capture))
        .route("/set-aperture", post(set_aperture))
        .route("/set-shutter-speed", post(set_shutter_speed(state, request)))
        .with_state(ServerState { cmd_tx });

    axum::Server::bind(&"192.168.1.25:8080".parse().unwrap())
        .serve(app.into_make_service())
        .await?;

    Ok(())
    
}

// endpoint sends a zoom by focal length request to the plane system
async fn set_focal_length(
    State(state): State<ServerState>,
    request: Json<ZoomRequestJSON>,
) -> Response {
    debug!("hit focal-length endpoint");
    let req = CameraRequest::Zoom(CameraZoomRequest::FocalLength{
        focal_length: request.focal_length
    });
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

// endpoint sends a zoom by levels ([0-60], where levels [30-60] are really digital zoom) request to the plane system
async fn set_level(
    State(state): State<ServerState>,
    request: Json<ZoomRequestJSON>,
) -> Response {
    debug!("hit level endpoint");
    let req = CameraRequest::Zoom(CameraZoomRequest::Level{
        level: request.level
    });
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

// endpoint sends a fractional shutter speed to the camera with numerator, denominator
async fn set_shutter_speed(
    State(state): State<ServerState>,
    request: Json<ShutterSpeedRequestJSON>,
) -> Response {
    debug!("hit level endpoint");
    let req = CameraSetRequest::ShutterSpeed { speed: (crate::ShutterSpeed::Fraction{numerator :request.numerator, denominator :request.denominator}) };
    let (ret_tx, ret_rx) = oneshot::channel();
    state.cmd_tx.send_async((CameraRequest::Set(req), ret_tx)).await;

    let response = match ret_rx.await {
        Ok(response) => response,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    match response {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}



// sends a request to change aperture to the camera with aperture (u16)
async fn set_aperture(
    State(state): State<ServerState>,
    request: Json<ApertureRequestJSON>,
) -> Response {
    debug!("hit level endpoint");
    let req = CameraSetRequest::Aperture { aperture: (crate::Aperture::Value(request.aperture)) };
    let (ret_tx, ret_rx) = oneshot::channel();
    state.cmd_tx.send_async((CameraRequest::Set(req), ret_tx)).await;

    let response = match ret_rx.await {
        Ok(response) => response,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    match response {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    }
}

// sends a request to the plane system to take a single image, what happens when modes are running? ask amin
async fn capture(State(state): State<ServerState>) -> Response {
    debug!("hit capture http endpoint");

    let req = CameraRequest::Capture {
        burst_duration: None,
        burst_high_speed: false,
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


// sends a request to the plane system to get the current zoom level (does this always work? has this ever been tested?)
async fn get_level(State(state): State<ServerState>) -> Response {
    debug!("hit get level http endpoint");

    let req = CameraRequest::Get(CameraGetRequest::ZoomLevel);
    let (ret_tx, ret_rx) = oneshot::channel();
    state.cmd_tx.send_async((req, ret_tx)).await;

    let response = match ret_rx.await {
        Ok(response) => response,
        Err(err) => return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
    };

    match response {
        Ok(CameraResponse::ZoomLevel(lvl)) => Json(LevelResponseJSON { /*whats best design here.. how to return the response.. */
            level : lvl
        }).into_response(),    
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response(),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "wrong type of response value recieved at get level endpoint").into_response(),
    }
} 


#[derive(Deserialize)]
struct ZoomRequestJSON {
    focal_length: f32,
    level: u8,

} 

#[derive(Deserialize, Serialize)]
struct LevelResponseJSON {
    level: u8,
} 


#[derive(Deserialize, Serialize)]
struct ShutterSpeedRequestJSON {
    numerator: u16,
    denominator: u16
} 


#[derive(Deserialize, Serialize)]
struct ApertureRequestJSON {
    aperture: u16
    
} 


