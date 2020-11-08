use std::{sync::Arc, time::Duration};

use anyhow::Context;
use cli_table::{format::CellFormat, Cell, Row, Table};
use humansize::{file_size_opts, FileSize};
use tokio::{
    sync::broadcast::{self, TryRecvError},
    task::spawn_blocking,
};

use crate::{
    cli::repl::{CameraCliCommand, CliCommand, CliResult},
    Channels,
};

use super::{interface::CameraInterface, state::CameraMessage};

pub struct CameraClient {
    iface: CameraInterface,
    channels: Arc<Channels>,
    cli: broadcast::Receiver<CliCommand>,
    interrupt: broadcast::Receiver<()>,
}

impl CameraClient {
    pub fn connect(channels: Arc<Channels>) -> anyhow::Result<Self> {
        let iface = CameraInterface::new().context("failed to create camera interface")?;

        let cli = channels.cli_cmd.subscribe();
        let interrupt = channels.interrupt.subscribe();

        Ok(CameraClient {
            iface,
            channels,
            cli,
            interrupt,
        })
    }

    pub fn init(&mut self) -> anyhow::Result<()> {
        info!("intializing camera");

        self.iface.connect()?;

        info!("initialized camera");

        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.init()?;

        loop {
            match self.interrupt.try_recv() {
                Ok(_) => break,
                Err(TryRecvError::Empty) => {}
                Err(_) => todo!("handle interrupt receiver lagging??"),
            }

            match self.cli.try_recv() {
                Ok(CliCommand::Camera(cmd)) => {
                    let result = self.exec(cmd).await;

                    match result {
                        Ok(result) => self.channels.cli_result.clone().send(result).await?,
                        Err(err) => {
                            error!("{}", err);
                            self.channels
                                .cli_result
                                .clone()
                                .send(CliResult::failure())
                                .await?
                        }
                    };
                }
                Ok(CliCommand::Exit) => break,
                Ok(_) | Err(_) => {}
            }

            tokio::time::delay_for(Duration::from_secs(1)).await;
        }

        Ok(())
    }

    async fn exec(&mut self, cmd: CameraCliCommand) -> anyhow::Result<CliResult> {
        match cmd {
            CameraCliCommand::ChangeDirectory { directory } => todo!(),
            CameraCliCommand::EnumerateDirectory { deep } => {
                let storage_ids = self
                    .iface
                    .storage_ids()
                    .context("could not get storage ids")?;

                let storage_infos = storage_ids
                    .iter()
                    .map(|&id| self.iface.storage_info(id).map(|info| (id, info)))
                    .collect::<Result<Vec<_>, _>>()
                    .context("failed to read storage information")?;

                let title_row = Row::new(vec![
                    Cell::new("id", Default::default()),
                    Cell::new("label", Default::default()),
                    Cell::new("capacity", Default::default()),
                    Cell::new("space", Default::default()),
                    Cell::new("accesss", Default::default()),
                ]);

                let mut table_rows = vec![title_row];

                table_rows.extend(storage_infos.into_iter().map(|(id, info)| {
                    let max_capacity_str =
                        info.max_capacity.file_size(file_size_opts::BINARY).unwrap();

                    let free_space_str = info
                        .free_space_in_bytes
                        .file_size(file_size_opts::BINARY)
                        .unwrap();

                    let access_str = match info.access_capability {
                        ptp::AccessType::Standard(cap) => {
                            let access_str = match cap {
                                ptp::StandardAccessType::ReadWrite => "rw",
                                ptp::StandardAccessType::ReadOnlyNoDelete => "r",
                                ptp::StandardAccessType::ReadOnly => "rd",
                            };

                            access_str.to_owned()
                        }
                        ptp::AccessType::Reserved(val) => format!("0x{:04x}", val),
                    };

                    Row::new(vec![
                        Cell::new(&format!("{:#?}", id), Default::default()),
                        Cell::new(&info.volume_label, Default::default()),
                        Cell::new(&max_capacity_str, Default::default()),
                        Cell::new(&free_space_str, Default::default()),
                        Cell::new(&access_str, Default::default()),
                    ])
                }));

                let table = Table::new(table_rows, cli_table::format::NO_BORDER_COLUMN_TITLE)
                    .context("could not create table")?;

                table.print_stdout().context("could not write table")?;

                Ok(CliResult::success())
            }
            CameraCliCommand::Capture => todo!(),
            CameraCliCommand::Zoom { level } => todo!(),
            CameraCliCommand::Download { file } => todo!(),
        }
    }
}
