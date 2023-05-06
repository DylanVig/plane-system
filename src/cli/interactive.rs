use clap::Parser;
use futures::{AsyncWriteExt, FutureExt};
use gimbal::GimbalResponse;
use ps_client::ChannelCommandSink;
use rustyline_async::{Readline, SharedWriter};
use tokio::{select, sync::oneshot};
use tokio_util::sync::CancellationToken;

use ps_gimbal as gimbal;
#[cfg(feature = "livestream")]
use ps_livestream::custom as ls;
use ps_main_camera as mc;

#[derive(Parser, Debug)]
#[command(no_binary_name = true, rename_all = "kebab-case")]
enum Command {
    #[command(subcommand)]
    #[command(name = "camera")]
    MainCamera(ps_main_camera::CameraRequest),

    #[cfg(feature = "livestream")]
    #[command(subcommand)]
    #[command(name = "livestream", alias = "ls")]
    LiveStream(ps_livestream::custom::LivestreamRequest),

    #[command(subcommand)]
    Gimbal(ps_gimbal::GimbalRequest),

    #[command(subcommand)]
    Mode(ps_modes::command::ModeRequest),

    Exit,
}

#[derive(Clone)]
pub struct CliChannels {
    pub camera_cmd_tx: Option<ChannelCommandSink<mc::CameraRequest, mc::CameraResponse>>,
    #[cfg(feature = "livestream")]
    pub livestream_cmd_tx:
        Option<ChannelCommandSink<ls::LivestreamRequest, ls::LivestreamResponse>>,
    pub gimbal_cmd_tx: Option<ChannelCommandSink<gimbal::GimbalRequest, gimbal::GimbalResponse>>,
    pub modes_cmd_tx:
        Option<ChannelCommandSink<ps_modes::command::ModeRequest, ps_modes::command::ModeResponse>>,
}

pub async fn run_interactive_cli(
    mut editor: Readline,
    mut stdout: SharedWriter,
    channels: CliChannels,
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

                        let request: Result<Command, _> = Parser::try_parse_from(line.split_ascii_whitespace());

                        let request = match request {
                            Ok(request) => request,
                            Err(err) => {
                                stdout.write_all(err.to_string().as_bytes()).await?;
                                continue;
                            },
                        };

                        editor.add_history_entry(line);

                        tokio::spawn(run_interactive_cmd(request, channels.clone(), cancellation_token.clone()));
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

async fn run_interactive_cmd(
    cmd: Command,
    channels: CliChannels,
    cancellation_token: CancellationToken,
) -> anyhow::Result<()> {
    let CliChannels {
        camera_cmd_tx,
        #[cfg(feature = "livestream")]
        livestream_cmd_tx,
        gimbal_cmd_tx,
        modes_cmd_tx,
    } = channels;

    match cmd {
        Command::MainCamera(request) => {
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

        #[cfg(feature = "livestream")]
        Command::LiveStream(request) => {
            if let Some(livestream_cmd_tx) = &livestream_cmd_tx {
                let (ret_tx, ret_rx) = oneshot::channel();

                if let Err(err) = livestream_cmd_tx.send_async((request, ret_tx)).await {
                    error!("livestream task did not accept command: {:#?}", err);
                }

                match ret_rx.await? {
                    Ok(response) => info!("{:?}", response),
                    Err(err) => error!("{:?}", err),
                };
            } else {
                error!("livestream task is not running");
            }
        }

        Command::Gimbal(request) => {
            if let Some(gimbal_cmd_tx) = &gimbal_cmd_tx {
                let (ret_tx, ret_rx) = oneshot::channel();

                info!("request = {request:?}");

                if let Err(err) = gimbal_cmd_tx.try_send((request, ret_tx)) {
                    error!("gimbal task did not accept command: {:#?}", err);
                }

                info!("awaiting gimbal response");

                match ret_rx.await? {
                    Ok(response) => info!("{:?}", response),
                    Err(err) => error!("{:?}", err),
                };
            } else {
                error!("gimbal task is not running");
            }
        }

        Command::Mode(request) => {
            if let Some(ps_modes_tx) = &ps_modes_cmd_tx {
                let (ret_tx, ret_rx) = oneshot::channel();

                info!("request = {request:?}");

                if let Err(err) = ps_modes_tx.try_send((request, ret_tx)) {
                    error!("modes task did not accept command: {:#?}", err);
                }

                match ret_rx.await? {
                    Ok(response) => info!("{:?}", response),
                    Err(err) => error!("{:?}", err),
                };
            } else {
                error!("modes task is not running");
            }
        }

        Command::Exit => {
            info!("exiting");
            cancellation_token.cancel();
        }
    };

    Ok(())
}
