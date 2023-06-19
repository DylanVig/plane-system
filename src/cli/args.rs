use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct MainArgs {
    /// The path to the config file for the plane system
    #[clap(long, short)]
    pub config: PathBuf,

    /// The path to a text file containing a list of commands to execute
    #[clap(long, short)]
    pub script: Option<PathBuf>,
}
