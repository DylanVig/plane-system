use std::sync::Arc;

use anyhow::Context;
use clap::AppSettings;
use structopt::StructOpt;
use tokio::sync::{broadcast, mpsc};

use crate::Channels;

#[derive(StructOpt, Debug, Clone)]
#[structopt(setting(AppSettings::NoBinaryName))]
#[structopt(rename_all = "kebab-case")]
pub enum CliCommand {
    Camera(CameraCliCommand),
    Exit,
}

#[derive(Debug, Clone)]
pub struct CliResult {
    new_current_directory: Option<String>,
    success: bool,
}

impl CliResult {
    pub fn success() -> Self {
        CliResult {
            new_current_directory: None,
            success: true
        }
    }

    pub fn failure() -> Self {
        CliResult {
            new_current_directory: None,
            success: false
        }
    }
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

pub async fn run(
    channels: Arc<Channels>,
    mut receiver: mpsc::Receiver<CliResult>,
) -> anyhow::Result<()> {
    let sender = &channels.cli_cmd;
    let mut rl = rustyline::Editor::<()>::new();
    let mut current_directory = "/".to_owned();

    loop {
        let current_prompt = format!("\n{}\nplane-system> ", current_directory);

        let line = rl
            .readline(&current_prompt)
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

        let result = receiver.recv().await.context("channel closed")?;

        if let Some(new_current_directory) = result.new_current_directory {
            current_directory = new_current_directory;
        }
    }

    Ok(())
}
