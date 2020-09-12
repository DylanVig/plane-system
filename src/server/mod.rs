use crate::{camera::Camera, client::camera::CameraClient};
use anyhow::Context;
use smol::lock::Mutex;
use std::sync::Arc;
use tide::{self, Request};

#[derive(Clone)]
struct ServerState {
    camera: Arc<Mutex<Camera>>,
}

pub async fn serve() -> Result<(), std::io::Error> {
    info!("initializing server");

    let state = ServerState {
        camera: CameraClient::new(),
    };

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
            let camera = req.state().camera.clone();

            if camera.lock().await.is_connected() {
                Ok("already connected")
            } else {
                smol::spawn(async {
                    if let Err(e) = connect_camera(camera).await {
                        error!("error while connecting to camera: {:#?}", e);
                    }
                })
                .detach();
                Ok("working")
            }
        });

    app.at("/disconnect")
        .get(|req: Request<ServerState>| async move {
            let camera = req.state().camera.clone();

            if !camera.lock().await.is_connected() {
                Ok("already disconnected")
            } else {
                smol::spawn(async {
                    if let Err(e) = disconnect_camera(camera).await {
                        error!("error while disconnecting from camera: {:#?}", e);
                    }
                })
                .detach();
                Ok("working")
            }
        });

    let address = "127.0.0.1:8080";
    info!("initialized server");
    info!("listening at {}", address);

    app.listen(address).await?;
    Ok(())
}

async fn connect_camera(camera: Arc<Mutex<Camera>>) -> anyhow::Result<()> {
    info!("connecting to camera");
    let mut camera = camera.lock().await;

    camera.connect().context("failed to connect to camera")?;

    info!("connected to camera");

    Ok(())
}

async fn disconnect_camera(camera: Arc<Mutex<Camera>>) -> anyhow::Result<()> {
    info!("disconnecting from to camera");

    Ok(())
}
