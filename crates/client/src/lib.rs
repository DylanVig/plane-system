use async_trait::async_trait;

use tokio_stream::Stream;

#[async_trait]
pub trait Context {
    fn subscribe<C>() -> C::Stream where C: EventClient;
    async fn command<C>(request: C::Request) -> C::Response where C: CommandClient;
}

pub trait EventClient {
    type Event;
    type Stream: Stream<Item = Self::Event>;

    fn subscribe(&self) -> Self::Stream;
}

#[async_trait]
pub trait CommandClient {
    type Request;
    type Response;

    async fn command(&self, request: Self::Request) -> Self::Response;
}

pub type Command<Req, Res> = (Req, tokio::sync::oneshot::Sender<anyhow::Result<Res>>);
pub type CommandSender<Req, Res> = tokio::sync::mpsc::Sender<Command<Req, Res>>;
pub type CommandReceiver<Req, Res> = tokio::sync::mpsc::Receiver<Command<Req, Res>>;

#[async_trait]
impl<Req: Send, Res: Send> CommandClient for CommandSender<Req, Res> {
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
