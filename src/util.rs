use std::{num::ParseIntError, time::Duration};

use futures::Future;
use tokio::sync::broadcast::{self, RecvError};

pub fn parse_hex_u32(src: &str) -> Result<u32, ParseIntError> {
    u32::from_str_radix(src, 16)
}

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

pub fn retry<F: FnMut() -> Result<T, E>, T, E>(
    times: usize,
    spacing: Option<Duration>,
    mut op: F,
) -> Result<T, E> {
    if times < 1 {
        panic!("retry called with times < 1");
    }

    let mut result = op();
    let mut tries = 1;

    while tries < times && result.is_err() {
        result = op();

        if let Some(spacing) = spacing {
            std::thread::sleep(spacing);
        }

        tries += 1;
    }

    result
}

pub async fn retry_delay<F: FnMut() -> Result<T, E>, T, E>(
    times: usize,
    spacing: Duration,
    mut op: F,
) -> Result<T, E> {
    if times < 1 {
        panic!("retry_delay called with times < 1");
    }

    let mut result = op();
    let mut tries = 1;

    while tries < times && result.is_err() {
        result = op();

        tokio::time::delay_for(spacing).await;

        tries += 1;
    }

    result
}

pub async fn retry_async<F: FnMut() -> Fut, Fut: Future<Output = Result<T, E>>, T, E>(
    times: usize,
    spacing: Option<Duration>,
    mut op: F,
) -> Result<T, E> {
    if times < 1 {
        panic!("retry_async called with times < 1");
    }

    let mut result = op().await;
    let mut tries = 1;

    while tries < times && result.is_err() {
        result = op().await;

        if let Some(spacing) = spacing {
            tokio::time::delay_for(spacing).await;
        }

        tries += 1;
    }

    result
}
