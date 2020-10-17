use std::sync::Arc;

use anyhow::Context;
use clap::AppSettings;
use structopt::StructOpt;
use tokio::sync::broadcast;

use crate::Channels;

#[derive(StructOpt, Debug, Clone)]
#[structopt(setting(AppSettings::NoBinaryName))]
pub enum CliCommand {
    TakeImage,
    Exit,
}

pub fn run(channels: Arc<Channels>) -> anyhow::Result<()> {
    let sender = &channels.cli;

    loop {
        let mut rl = rustyline::Editor::<()>::new();
        let line = rl
            .readline("plane-system> ")
            .context("failed to read line")?;

        let cmd: CliCommand =
            CliCommand::from_iter_safe(line.split_ascii_whitespace()).context("invalid command")?;
        

        match cmd {
            CliCommand::TakeImage => {
                info!("taking image");
            }
            CliCommand::Exit => {
                info!("exiting");
                break;
            }
        }
    }

    Ok(())
}
