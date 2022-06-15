use std::{num::ParseIntError, time::Duration};

use futures::Future;
use tokio::sync::broadcast::{self, error::RecvError};

// by default, chrono will format with 10 or so fractional digits but python's
// builtin iso datetime parser only supports 6 digits, so this makes it a pain
// for postprocessing
pub const ISO_8601_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%.6f%:z";

pub fn serialize_time<S>(
    me: &chrono::DateTime<chrono::Local>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::ser::Serializer,
{
    serializer.collect_str(&me.format(ISO_8601_FORMAT).to_string())
}

pub fn parse_hex_u32(src: &str) -> Result<u32, ParseIntError> {
    u32::from_str_radix(src, 16)
}

#[doc(hidden)]
pub(crate) async fn run_loop_impl<C>(
    loop_fut: impl Future<Output = Result<(), anyhow::Error>>,
    cancellation_fut: impl Future<Output = C>,
) -> Option<anyhow::Result<(), anyhow::Error>> {
    futures::pin_mut!(loop_fut);
    futures::pin_mut!(cancellation_fut);

    match futures::future::select(loop_fut, cancellation_fut).await {
        futures::future::Either::Left((out, _)) => Some(out),
        futures::future::Either::Right(_) => None,
    }
}

/// Runs `loop_fut` until `cancellation_fut` resolves. If `loop_fut` exits with
/// an error, then the error is returned from the current method.
macro_rules! run_loop {
    ($loop_fut: expr, $cancellation_fut: expr) => {
        if let Some(res) = $crate::util::run_loop_impl($loop_fut, $cancellation_fut).await {
            res?
        }
    };
}

pub(crate) use run_loop;

pub fn spawn_with_name<T: Send + 'static>(
    name: &str,
    task: impl Future<Output = T> + Send + 'static,
) -> tokio::task::JoinHandle<T> {
    #[cfg(tokio_unstable)]
    return tokio::task::Builder::new().name(name).spawn(task);

    #[cfg(not(tokio_unstable))]
    return tokio::task::spawn(task);
}

pub fn spawn_blocking_with_name<T: Send + 'static>(
    name: &str,
    task: impl FnOnce() -> T + Send + 'static,
) -> tokio::task::JoinHandle<T> {
    #[cfg(tokio_unstable)]
    return tokio::task::Builder::new().name(name).spawn_blocking(task);

    #[cfg(not(tokio_unstable))]
    return tokio::task::spawn_blocking(task);
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
            tokio::time::sleep(spacing).await;
        }

        tries += 1;
    }

    result
}
