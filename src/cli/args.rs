use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
pub struct MainArgs {
    /// The path to the config file for the plane system. Will use
    /// plane-system.json by default.
    #[structopt(parse(from_os_str))]
    pub config: Option<PathBuf>,
}
