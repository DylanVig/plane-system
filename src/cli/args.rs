use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct MainArgs {
    /// The path to the config file for the plane system. Will use
    /// plane-system.json by default.
    #[clap(parse(from_os_str), long, short)]
    pub config: Option<PathBuf>,
}
