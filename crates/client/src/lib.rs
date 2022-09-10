use async_trait::async_trait;

use tokio_stream::Stream;
use tokio_util::sync::CancellationToken;

pub trait EventSource {
    type Event;
    type Stream: Stream<Item = Self::Event>;

    fn subscribe(&self) -> Self::Stream;
}

#[async_trait]
pub trait CommandSink {
    type Request;
    type Response;

    async fn command(&self, request: Self::Request) -> Self::Response;
}

#[async_trait]
pub trait Task {
    fn name() -> &'static str;

    async fn run(self, cancel: CancellationToken) -> anyhow::Result<()>;
}

pub type Command<Req, Res> = (Req, tokio::sync::oneshot::Sender<anyhow::Result<Res>>);
pub type ChannelCommandSink<Req, Res> = tokio::sync::mpsc::Sender<Command<Req, Res>>;
pub type ChannelCommandSource<Req, Res> = tokio::sync::mpsc::Receiver<Command<Req, Res>>;

#[async_trait]
impl<Req: Send, Res: Send> CommandSink for ChannelCommandSink<Req, Res> {
    type Request = Req;
    type Response = anyhow::Result<Res>;

    async fn command(&self, request: Self::Request) -> Self::Response {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if let Err(_) = self.send((request, tx)).await {
            anyhow::bail!("could not send command");
        }
        rx.await?
    }
}
