use std::sync::Arc;

use colored::Colorize;

use clap::{Parser, Subcommand};
use humansize::FileSize;
use prettytable::{cell, row, Table};
use tracing::Level;

use crate::{
    camera::main::{CameraCommandRequest, CameraCommandResponse, CameraSaveMode},
    gimbal::GimbalRequest,
    gs::GroundServerRequest,
    Channels, Command,
};

#[cfg(feature = "gstreamer")]
use crate::camera::aux::{save::SaveRequest, stream::StreamRequest};

#[derive(Parser, Debug)]
#[clap(setting(clap::AppSettings::NoBinaryName))]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
#[clap(rename_all = "kebab-case")]
enum Commands {
    #[clap(subcommand)]
    MainCamera(CameraCommandRequest),
    #[clap(subcommand)]
    Gimbal(GimbalRequest),
    #[clap(subcommand)]
    GroundServer(GroundServerRequest),
    Exit,

    #[cfg(feature = "gstreamer")]
    #[clap(subcommand)]
    AuxCamera(AuxCameraRequest),
}

#[cfg(feature = "gstreamer")]
#[derive(Subcommand, Debug)]
enum AuxCameraRequest {
    #[clap(subcommand)]
    Stream(StreamRequest),
    #[clap(subcommand)]
    Save(SaveRequest),
}

#[tracing::instrument]
pub async fn run(channels: Arc<Channels>) -> anyhow::Result<()> {
    let mut interrupt_recv = channels.interrupt.subscribe();

    let repl_fut = async move {
        let mut rl_editor = rustyline::Editor::<()>::new();

        loop {
            let request = match tokio::task::block_in_place(|| {
                rl_editor.readline(&"\n\nplane-system> ".bright_white())
            }) {
                Ok(line) => {
                    let request: Result<Cli, _> =
                        Parser::try_parse_from(line.split_ascii_whitespace());

                    match request {
                        Ok(request) => {
                            rl_editor.add_history_entry(line);
                            request.command
                        }
                        Err(err) => {
                            println!("{}", err);
                            continue;
                        }
                    }
                }
                Err(err) => match err {
                    rustyline::error::ReadlineError::Interrupted => Commands::Exit,
                    err => break Err::<_, anyhow::Error>(err.into()),
                },
            };

            let span = span!(Level::TRACE, "command parsed", ?request);
            let _enter = span.enter();

            match request {
                Commands::MainCamera(request) => {
                    let (cmd, chan) = Command::new(request);
                    if let Err(err) = channels.camera_cmd.clone().send(cmd) {
                        error!("camera client not available: {}", err);
                        continue;
                    }

                    trace!("command sent, awaiting response");
                    let result = chan.await?;
                    trace!("command completed, received response");

                    match result {
                        Ok(response) => format_camera_response(response),
                        Err(err) => println!("{}", format!("error: {}", err).red()),
                    };
                }
                Commands::Gimbal(request) => {
                    let (cmd, chan) = Command::new(request);
                    if let Err(err) = channels.gimbal_cmd.clone().send(cmd) {
                        error!("gimbal client not available: {}", err);
                        continue;
                    }
                    let _ = chan.await?;
                }
                Commands::GroundServer(request) => match request {},
                Commands::Exit => {
                    let _ = channels.interrupt.send(());
                    break Ok(());
                }
                #[cfg(feature = "gstreamer")]
                Commands::AuxCamera(request) => match request {
                    AuxCameraRequest::Stream(request) => {
                        let (cmd, chan) = Command::new(request);
                        if let Err(err) = channels.stream_cmd.clone().send(cmd) {
                            error!("stream client not available: {}", err);
                            continue;
                        }
                        let _ = chan.await?;
                    }
                    AuxCameraRequest::Save(request) => {
                        let (cmd, chan) = Command::new(request);
                        if let Err(err) = channels.save_cmd.clone().send(cmd) {
                            error!("save client not available: {}", err);
                            continue;
                        }
                        let _ = chan.await?;
                    }
                },
                _ => {
                    error!("this command is not available");
                }
            };
        }
    };

    let interrupt_fut = interrupt_recv.recv();

    futures::pin_mut!(repl_fut);
    futures::pin_mut!(interrupt_fut);

    match futures::future::select(interrupt_fut, repl_fut).await {
        futures::future::Either::Left((_, _repl_fut)) => {}
        futures::future::Either::Right((repl_result, _)) => repl_result?,
    }

    Ok(())
}

fn table_format() -> prettytable::format::TableFormat {
    prettytable::format::FormatBuilder::new()
        .column_separator('|')
        .borders('|')
        .separators(
            &[
                prettytable::format::LinePosition::Top,
                prettytable::format::LinePosition::Bottom,
            ],
            prettytable::format::LineSeparator::new('-', '+', '+', '+'),
        )
        .padding(1, 1)
        .build()
}

fn format_camera_response(response: CameraCommandResponse) -> () {
    match response {
        CameraCommandResponse::Unit => println!("done"),

        CameraCommandResponse::Data { data } => {
            let size = data
                .len()
                .file_size(humansize::file_size_opts::BINARY)
                .unwrap();

            println!("received {} of data", size);
        }

        CameraCommandResponse::Download { name: path } => {
            println!("received file: {}", path);
        }

        CameraCommandResponse::StorageInfo { storages } => {
            let mut table = Table::new();
            table.add_row(row![
                "id",
                "label",
                "filesystem",
                "storage type",
                "capacity",
                "free space",
                "access"
            ]);

            for (id, info) in storages.into_iter() {
                let capacity = info
                    .max_capacity
                    .file_size(humansize::file_size_opts::BINARY)
                    .unwrap();

                let free_space = info
                    .free_space_in_bytes
                    .file_size(humansize::file_size_opts::BINARY)
                    .unwrap();

                let access = match info.access_capability {
                    ptp::AccessType::Standard(s) => match s {
                        ptp::StandardAccessType::ReadWrite => "r+w",
                        ptp::StandardAccessType::ReadOnlyNoDelete => "r+d",
                        ptp::StandardAccessType::ReadOnly => "r",
                    }
                    .to_string(),
                    ptp::AccessType::Reserved(r) => format!("0x{:04x}", r),
                };

                let volume_label = info.volume_label;

                let fs_type = match info.filesystem_type {
                    ptp::FilesystemType::Standard(s) => match s {
                        ptp::StandardFilesystemType::Undefined => "unknown",
                        ptp::StandardFilesystemType::GenericFlat => "flat",
                        ptp::StandardFilesystemType::GenericHierarchical => "hierarchical",
                        ptp::StandardFilesystemType::DCF => "dcf",
                    }
                    .to_string(),
                    ptp::FilesystemType::Reserved(r) | ptp::FilesystemType::Vendor(r) => {
                        format!("0x{:04x}", r)
                    }
                };

                let storage_type = match info.storage_type {
                    ptp::StorageType::Standard(s) => match s {
                        ptp::StandardStorageType::Undefined => "unknown",
                        ptp::StandardStorageType::FixedRom => "fixed rom",
                        ptp::StandardStorageType::RemovableRom => "removable rom",
                        ptp::StandardStorageType::FixedRam => "fixed ram",
                        ptp::StandardStorageType::RemovableRam => "removable ram",
                    }
                    .to_string(),
                    ptp::StorageType::Reserved(r) => format!("0x{:04x}", r),
                };

                table.add_row(row![
                    id,
                    volume_label,
                    fs_type,
                    storage_type,
                    capacity,
                    free_space,
                    access
                ]);
            }

            table.set_format(table_format());
            table.printstd();
        }

        CameraCommandResponse::ObjectInfo { objects } => {
            let mut table = Table::new();

            table.add_row(row![
                "handle",
                "filename",
                "type",
                "compressed size",
                "capture date",
                "modified date",
                "dimensions"
            ]);

            for (id, info) in objects.into_iter() {
                let file_name = info.filename;

                let size = info
                    .object_compressed_size
                    .file_size(humansize::file_size_opts::BINARY)
                    .unwrap();

                let dimensions = format!("{}x{}", info.image_pix_width, info.image_pix_height);

                let file_type = match info.object_format {
                    ptp::ObjectFormatCode::Standard(s) => match s {
                        ptp::StandardObjectFormatCode::Undefined
                        | ptp::StandardObjectFormatCode::UndefinedReserved
                        | ptp::StandardObjectFormatCode::UndefinedReserved2
                        | ptp::StandardObjectFormatCode::UndefinedImage
                        | ptp::StandardObjectFormatCode::UndefinedNonImage => "unknown".to_string(),
                        ptp::StandardObjectFormatCode::Association => match info.association_type {
                            ptp::AssociationCode::Standard(s) => match s {
                                ptp::StandardAssociationCode::Undefined => "association",
                                ptp::StandardAssociationCode::GenericFolder => "folder",
                                ptp::StandardAssociationCode::Album => "album",
                                ptp::StandardAssociationCode::TimeSequence => "time sequence",
                                ptp::StandardAssociationCode::PanoramicHorizontal => "h. panorama",
                                ptp::StandardAssociationCode::PanoramicVertical => "v. panorama",
                                ptp::StandardAssociationCode::Panoramic2D => "2d panorama",
                                ptp::StandardAssociationCode::AncillaryData => "extra data",
                            }
                            .to_string(),
                            ptp::AssociationCode::Reserved(r) | ptp::AssociationCode::Vendor(r) => {
                                format!("0x{:04x}", r)
                            }
                        },
                        ptp::StandardObjectFormatCode::Script => "script".to_string(),
                        ptp::StandardObjectFormatCode::Executable => "executable".to_string(),
                        ptp::StandardObjectFormatCode::Text => "text".to_string(),
                        ptp::StandardObjectFormatCode::Html => "html".to_string(),
                        ptp::StandardObjectFormatCode::Dpof => "dpof".to_string(),
                        ptp::StandardObjectFormatCode::Aiff => "aiff".to_string(),
                        ptp::StandardObjectFormatCode::Wav => "wav".to_string(),
                        ptp::StandardObjectFormatCode::Mp3 => "mp3".to_string(),
                        ptp::StandardObjectFormatCode::Avi => "avi".to_string(),
                        ptp::StandardObjectFormatCode::Mpeg => "mpeg".to_string(),
                        ptp::StandardObjectFormatCode::Asf => "asf".to_string(),
                        ptp::StandardObjectFormatCode::ExifJpeg => "exif-jpeg".to_string(),
                        ptp::StandardObjectFormatCode::TiffEp => "tiff-ep".to_string(),
                        ptp::StandardObjectFormatCode::FlashPix => "flashpix".to_string(),
                        ptp::StandardObjectFormatCode::Bmp => "bmp".to_string(),
                        ptp::StandardObjectFormatCode::Ciff => "ciff".to_string(),
                        ptp::StandardObjectFormatCode::Gif => "gif".to_string(),
                        ptp::StandardObjectFormatCode::Jfif => "jfif".to_string(),
                        ptp::StandardObjectFormatCode::Pcd => "pcd".to_string(),
                        ptp::StandardObjectFormatCode::Pict => "pict".to_string(),
                        ptp::StandardObjectFormatCode::Png => "png".to_string(),
                        ptp::StandardObjectFormatCode::Tiff => "tiff".to_string(),
                        ptp::StandardObjectFormatCode::TiffIt => "tiff-it".to_string(),
                        ptp::StandardObjectFormatCode::Jp2 => "jp2".to_string(),
                        ptp::StandardObjectFormatCode::Jpx => "jpx".to_string(),
                    },
                    ptp::ObjectFormatCode::Reserved(r) | ptp::ObjectFormatCode::Vendor(r) => {
                        format!("0x{:04x}", r)
                    }
                    ptp::ObjectFormatCode::ImageOnly => "image".to_string(),
                };

                table.add_row(row![
                    id,
                    file_name,
                    file_type,
                    size,
                    info.capture_date,
                    info.modification_date,
                    dimensions
                ]);
            }

            table.set_format(table_format());
            table.printstd();
        }

        CameraCommandResponse::ZoomLevel(zoom_level) => {
            println!("zoom level: {}", zoom_level);
        }
        CameraCommandResponse::SaveMode(save_mode) => match save_mode {
            CameraSaveMode::HostDevice => {
                println!("saving to host device");
            }
            CameraSaveMode::MemoryCard1 => {
                println!("saving to camera memory");
            }
        },
        CameraCommandResponse::ExposureMode(exposure_mode) => {
            println!("exposure mode: {:?}", exposure_mode);
        }
        CameraCommandResponse::OperatingMode(operating_mode) => {
            println!("operating mode: {:?}", operating_mode);
        }
        CameraCommandResponse::FocusMode(focus_mode) => {
            println!("focus mode: {:?}", focus_mode);
        }
        CameraCommandResponse::CcInterval(interval) => {
            println!("continuous capture interval: {:?}", interval);
        }
    }
}
