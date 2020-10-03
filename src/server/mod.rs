use crate::{
    client::camera::CameraClient, interface::camera::CameraInterface,
};
use anyhow::Context;
use smol::lock::Mutex;
use std::sync::Arc;
use tide::{self, Request};

#[derive(Clone)]
struct ServerState {
    camera: CameraClient,
}

pub async fn serve() -> Result<(), std::io::Error> {
    info!("initializing server");

    let camera_interface = Arc::new(CameraInterface::new());

    let state = ServerState {
        camera: camera_interface.create_client(),
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
            todo!();
            Ok(tide::Response::new(200))
        });

    app.at("/disconnect")
        .get(|req: Request<ServerState>| async move {
            todo!();
            Ok(tide::Response::new(200))
        });

    let address = "127.0.0.1:8080";
    info!("initialized server");
    info!("listening at {}", address);

    app.listen(address).await?;
    Ok(())
}

async fn connect_camera(camera: Arc<Mutex<CameraInterface>>) -> anyhow::Result<()> {
    info!("connecting to camera");
    let mut camera = camera.lock().await;

    camera.connect().await.context("failed to connect to camera")?;

    info!("connected to camera");

    Ok(())
}

async fn disconnect_camera(camera: Arc<Mutex<CameraInterface>>) -> anyhow::Result<()> {
    info!("disconnecting from to camera");

    Ok(())
}
