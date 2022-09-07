use core::{marker::Send, pin::Pin};

use async_trait::async_trait;
use futures::Future;
use tokio_stream::Stream;
use tokio_util::sync::CancellationToken;

pub trait Controller {
    type Client: Client;
    type Runner: Runner;

    fn create() -> (Self::Client, Self::Runner);
}

pub trait Client {}

#[async_trait]
pub trait Runner {
    async fn run(self, cancel: CancellationToken) -> anyhow::Result<()>;
}

impl<
        Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
        F: FnOnce(CancellationToken) -> Fut,
    > Runner for F
{
    fn run<'fut>(
        self,
        cancel: CancellationToken,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'fut>> {
        Box::pin(self(cancel))
    }
}

pub trait EventClient: Client {
    type Event;
    type Stream: Stream<Item = Self::Event>;

    fn subscribe(&self) -> Self::Stream;
}

#[async_trait]
pub trait CommandClient: Client {
    type Request;
    type Response;

    async fn command(&self, request: Self::Request) -> Self::Response;
}
