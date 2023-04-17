use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct MainArgs {
    /// The path to the config file for the plane system. Will use
    /// plane-system.json by default.
    #[clap(long, short)]
    pub config: PathBuf,
}
