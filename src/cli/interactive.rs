use clap::Parser;
use futures::{AsyncWriteExt, FutureExt};
use ps_client::ChannelCommandSink;
use rustyline_async::{Readline, SharedWriter};
use tokio::{select, sync::oneshot};
use tokio_util::sync::CancellationToken;

#[derive(Parser, Debug)]
#[clap(setting(clap::AppSettings::NoBinaryName))]
#[clap(rename_all = "kebab-case")]
enum Commands {
    #[clap(subcommand)]
    #[clap(name = "camera")]
    MainCamera(ps_main_camera::CameraRequest),
    Exit,
}

pub async fn run_interactive_cli(
    mut editor: Readline,
    mut stdout: SharedWriter,
    camera_cmd_tx: Option<
        ChannelCommandSink<ps_main_camera::CameraRequest, ps_main_camera::CameraResponse>,
    >,
    cancellation_token: CancellationToken,
) -> anyhow::Result<()> {
    loop {
        select! {
            _ = cancellation_token.cancelled() => {
                break;
            }
            result = editor.readline().fuse() => {
                match result {
                    Ok(line) => {
                        stdout.write_all(format!("ps> {}\n", line).as_bytes()).await?;

                        let request: Result<Commands, _> = Parser::try_parse_from(line.split_ascii_whitespace());

                        let request = match request {
                            Ok(request) => request,
                            Err(err) => {
                                stdout.write_all(err.to_string().as_bytes()).await?;
                                continue;
                            },
                        };

                        editor.add_history_entry(line);

                        match request {
                            Commands::MainCamera(request) => {
                                if let Some(camera_cmd_tx) = &camera_cmd_tx {
                                    let (ret_tx, ret_rx) = oneshot::channel();
                                    if let Err(err) = camera_cmd_tx.send_async((request, ret_tx)).await {
                                        error!("camera task did not accept command: {:#?}", err);
                                    }
                                    match ret_rx.await? {
                                        Ok(response) => info!("{:?}", response),
                                        Err(err) => error!("{:?}", err),
                                    };
                                } else {
                                    error!("camera task is not running");
                                }
                            }

                            Commands::Exit => {
                                info!("exiting");
                                cancellation_token.cancel();
                            }
                        };
                    }
                    Err(err) => {
                        error!("interactive error: {:#?}", err);
                        break;
                    }
                };
            }
        }
    }

    cancellation_token.cancel();

    Ok(())
}
