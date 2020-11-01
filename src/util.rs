use tokio::sync::broadcast::{self, RecvError};

/// This is an extension trait for channel receivers.
#[async_trait]
pub(crate) trait ReceiverExt<T: Clone + Send> {
    /// Allows the user to get the first available value from the channel
    /// receiver, ignoring RecvError::Lagged. Will return None if the channel is
    /// closed.
    async fn recv_skip(&mut self) -> Option<T>;
}

#[async_trait]
impl<T: Clone + Send> ReceiverExt<T> for broadcast::Receiver<T> {
    async fn recv_skip(&mut self) -> Option<T> {
        loop {
            match self.recv().await {
                Ok(message) => break Some(message),
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break None,
            }
        }
    }
}
