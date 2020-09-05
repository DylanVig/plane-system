use tide;

pub async fn serve() -> Result<(), std::io::Error> {
    info!("initializing server");

    let mut app = tide::new();
    app.at("/").get(|_| async { Ok("hello world") });

    let address = "127.0.0.1:8080";
    info!("initialized server");
    info!("listening at {:#?}", address);

    app.listen(address).await?;
    Ok(())
}
