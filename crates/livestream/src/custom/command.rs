use clap::Subcommand;

#[derive(Subcommand, Debug, Clone)]
#[command(rename_all = "kebab-case")]
pub enum LivestreamRequest {
    Start { name: String },
    Stop { name: String },
}

pub type LivestreamResponse = ();
