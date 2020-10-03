use anyhow::Context;
use std::sync::Arc;
use tide::{self, Request};

#[derive(Clone)]
struct ServerState {
}

pub async fn serve() -> Result<(), std::io::Error> {
    info!("initializing server");

    let state = ServerState { };

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

    app.at("/api")
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
