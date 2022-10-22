use std::time::Duration;

use futures::Future;

pub async fn retry_async<F: FnMut() -> Fut, Fut: Future<Output = Result<T, E>>, T, E>(
    times: usize,
    spacing: Option<Duration>,
    mut op: F,
) -> Result<T, E> {
    if times < 2 {
        panic!("retry_async called with times < 2");
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
