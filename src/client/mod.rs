use smol::channel::{Receiver, Sender};

pub mod camera;
pub mod gimbal;
pub mod pixhawk;

pub struct Channels<Request: Send, Response: Send> {
    request_channel: (Sender<Request>, Receiver<Request>),
    response_channel: (Sender<Response>, Receiver<Response>),
}

#[async_trait]
pub trait Interface: Sized + Send + Sync + 'static {
    type Client;
    type Request: Send + Sized;
    type Response: Send + Sync + Sized + 'static;

    fn new_client(&self) -> Self::Client;

    /// Starts a task that will run the Pixhawk.
    fn run(self) -> smol::Task<anyhow::Result<()>> {
        smol::spawn(async move {
            let channels = self.channels();
            let (message_broadcaster, _) = channels.response_channel;
            let (_, message_terminal) = channels.request_channel;

            loop {
                if let Some(message) = self.recv().await? {
                    message_broadcaster.send(message).await?;
                }

                if !message_terminal.is_empty() {
                    let message = message_terminal.recv().await?;
                    self.send(message).await?;
                }
            }
        })
    }

    fn channels(&self) -> &Channels<Self::Request, Self::Response>;
    async fn recv(&self) -> anyhow::Result<Option<Self::Response>>;
    async fn send(&self, request: Self::Request) -> anyhow::Result<()>;
}
