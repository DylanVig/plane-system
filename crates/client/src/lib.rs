use async_trait::async_trait;

use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

#[async_trait]
pub trait CommandSink {
    type Request;
    type Response;

    async fn command(&self, request: Self::Request) -> Self::Response;
}

#[async_trait]
pub trait Task {
    fn name(&self) -> &'static str;

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()>;
}

pub type Command<Req, Res> = (Req, oneshot::Sender<anyhow::Result<Res>>);
pub type ChannelCommandSink<Req, Res> = flume::Sender<Command<Req, Res>>;
pub type ChannelCommandSource<Req, Res> = flume::Receiver<Command<Req, Res>>;

#[async_trait]
impl<Req: Send, Res: Send> CommandSink for ChannelCommandSink<Req, Res> {
    type Request = Req;
    type Response = anyhow::Result<Res>;

    async fn command(&self, request: Self::Request) -> Self::Response {
        let (tx, rx) = oneshot::channel();
        if let Err(_) = self.send_async((request, tx)).await {
            anyhow::bail!("could not send command");
        }
        rx.await?
    }
}
