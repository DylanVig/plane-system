use std::sync::Arc;

use anyhow::Context;
use structopt::StructOpt;
use tokio::sync::mpsc;

use crate::{Channels, command::{CommandData, ResponseData}};


pub async fn run(
    channels: Arc<Channels>,
) -> anyhow::Result<()> {
    let sender = &channels.cmd;
    let receiver = channels.response.subscribe();

    let mut rl = rustyline::Editor::<()>::new();
    let mut current_directory = "/".to_owned();

    loop {
        let current_prompt = format!("\n{}\nplane-system> ", current_directory);

        let line = rl
            .readline(&current_prompt)
            .context("failed to read line")?;

        trace!("got line: {:#?}", line);

        let cmd = match <CommandData as StructOpt>::from_iter_safe(line.split_ascii_whitespace()) {
            Ok(cmd) => cmd,
            Err(err) => {
                println!("{}", err.message);
                continue;
            }
        };

        trace!("got command: {:#?}", cmd);

        let _ = sender.send(cmd);

        let result = receiver.recv().await.context("channel closed")?;
    }

    Ok(())
}
