use anyhow::anyhow;
use async_trait::async_trait;

use tokio::{sync::oneshot, task::JoinSet};
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
    fn name(&self) -> &'static str;

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()>;
}

/// A [`Task`] that drives multiple sub-`Task`s.
pub struct MultiTask {
    name: &'static str,
    subtasks: Vec<Box<dyn Task + Send>>,
}

#[async_trait]
impl Task for MultiTask {
    fn name(&self) -> &'static str {
        self.name
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let mut js = JoinSet::new();
        for task in self.subtasks {
            #[cfg(tokio_unstable)]
            let _ = js
                .build_task()
                .name(task.name())
                .spawn(Box::new(task.run(cancel.child_token())));

            #[cfg(not(tokio_unstable))]
            let _ = js.spawn(Box::new(task.run(cancel.child_token())));
        }

        let mut errors = Vec::new();

        while let Some(res) = js.join_next().await {
            match res {
                Ok(res) => match res {
                    Ok(()) => {}
                    Err(err) => errors.push(err),
                },
                Err(err) => {
                    if err.is_panic() {
                        std::panic::resume_unwind(err.into_panic());
                    }

                    // otherwise, it means the task was cancelled, but we didn't
                    // write any code that cancels tasks, so that should never
                    // happen
                }
            }
        }

        match errors.len() {
            0 => Ok(()),
            1 => Err(errors.pop().unwrap()),
            _ => Err(anyhow!(
                "failed with multiple errors:\n\n{}",
                errors
                    .into_iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("\n\n")
            )),
        }
    }
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
