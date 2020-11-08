use std::sync::Arc;

use anyhow::Context;
use clap::AppSettings;
use structopt::StructOpt;
use tokio::sync::broadcast;

use crate::Channels;

#[derive(StructOpt, Debug, Clone)]
#[structopt(setting(AppSettings::NoBinaryName))]
#[structopt(rename_all = "kebab-case")]
pub enum CliCommand {
    Camera(CameraCliCommand),
    Exit,
}

#[derive(StructOpt, Debug, Clone)]
pub enum CameraCliCommand {
    #[structopt(name = "cd")]
    ChangeDirectory {
        directory: String,
    },

    #[structopt(name = "ls")]
    EnumerateDirectory {
        #[structopt(short, long)]
        deep: bool,
    },

    Capture,

    Zoom {
        level: u8,
    },

    Download {
        file: Option<String>,
    },
}

pub fn run(channels: Arc<Channels>) -> anyhow::Result<()> {
    let sender = &channels.cli;
    let mut rl = rustyline::Editor::<()>::new();

    loop {
        let line = rl
            .readline("\n\nplane-system> ")
            .context("failed to read line")?;

        trace!("got line: {:#?}", line);

        let cmd = match <CliCommand as StructOpt>::from_iter_safe(line.split_ascii_whitespace()) {
            Ok(cmd) => cmd,
            Err(err) => {
                println!("{}", err.message);
                continue;
            }
        };

        trace!("got command: {:#?}", cmd);

        let _ = sender.send(cmd);
    }

    Ok(())
}
