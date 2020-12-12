// real gimbal
pub mod hardware;

// virtual gimbal
pub mod software;

pub use hardware::*;
pub use software::*;

use serde::{Deserialize, Serialize};
use simplebgc::{IncomingCommand, OutgoingCommand};

pub trait GimbalInterface {
    fn new() -> anyhow::Result<Self>
    where
        Self: Sized;

    fn send_command(&mut self, cmd: OutgoingCommand) -> anyhow::Result<()>;

    fn recv_command(&mut self) -> anyhow::Result<Option<IncomingCommand>>;
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
pub enum GimbalKind {
    Hardware,
    Software,
}
