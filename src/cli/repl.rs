use std::sync::Arc;

use anyhow::Context;
use structopt::StructOpt;
use tokio::sync::mpsc;

use crate::{Channels, camera::CameraRequest, Command};

#[derive(StructOpt, Debug)]
enum ReplRequest {
    Camera(CameraRequest)
}

pub async fn run(
    channels: Arc<Channels>,
) -> anyhow::Result<()> {
    let mut rl = rustyline::Editor::<()>::new();
    let mut current_directory = "/".to_owned();

    loop {
        let current_prompt = format!("\n{}\nplane-system> ", current_directory);

        let line = rl
            .readline(&current_prompt)
            .context("failed to read line")?;

        trace!("got line: {:#?}", line);

        let request = match <ReplRequest as StructOpt>::from_iter_safe(line.split_ascii_whitespace()) {
            Ok(cmd) => cmd,
            Err(err) => {
                println!("{}", err.message);
                continue;
            }
        };

        trace!("got command: {:#?}", request);

        let response = match request {
            ReplRequest::Camera(request) => {
                let (cmd, chan) = Command::new(request);
                channels.camera_cmd.clone().send(cmd).await?;
                chan.await
            }
        };
    }

    Ok(())
}
