use crate::{interface::GimbalInterface};

pub struct GimbalTask {
    iface: Box<dyn GimbalInterface + Send>,
    cmd: flume::Receiver<GimbalCommand>,
}

impl GimbalTask {
    pub fn connect_with_path<P: AsRef<Path>>(
        cmd: flume::Receiver<GimbalCommand>,
        path: P,
    ) -> anyhow::Result<Self> {
        let iface = HardwareGimbalInterface::with_path(path)
            .context("failed to create gimbal interface")?;

        Ok(Self {
            iface: Box::new(iface),
            cmd,
        })
    }

    pub fn connect(
        cmd: flume::Receiver<GimbalCommand>,
        kind: GimbalKind,
    ) -> anyhow::Result<Self> {
        let iface: Box<dyn GimbalInterface + Send> = match kind {
            GimbalKind::Hardware { protocol: _ } => Box::new(
                HardwareGimbalInterface::new()
                    .context("failed to create hardware gimbal interface")?,
            ),
            GimbalKind::Software => Box::new(
                SoftwareGimbalInterface::new()
                    .context("failed to create software gimbal interface")?,
            ),
        };

        Ok(Self {
            iface,
            cmd,
        })
    }

    pub fn init(&self) -> anyhow::Result<()> {
        trace!("initializing gimbal");
        Ok(())
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.init()?;

        let mut interrupt_recv = self.channels.interrupt.subscribe();
        let interrupt_fut = interrupt_recv.recv().fuse();
        futures::pin_mut!(interrupt_fut);

        loop {
            futures::select! {
                cmd = self.cmd.recv_async().fuse() => {
                    if let Ok(cmd) = cmd {
                        let result = self.exec(cmd.request()).await;
                        let _ = cmd.respond(result);
                    }
                }
                _ = interrupt_fut => break,
            }
        }

        Ok(())
    }

    async fn exec(&mut self, cmd: &GimbalRequest) -> anyhow::Result<GimbalResponse> {
        match cmd {
            GimbalRequest::Control { roll, pitch } => {
                self.iface.control_angles(*roll, *pitch).await?
            }
        }

        Ok(GimbalResponse::Unit)
    }
}

#[async_trait]
impl Task for GimbalTask{
    fn name(&self) -> &'static str {
        "gimbal"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let Self {
            iface,
            cmd,
            ..
        } = *self;

        let loop_fut = async move{

        };

        select! {
            _ = cancel.cancelled() => {}
            res = loop_fut => { res? }
          }
  
        Ok(())

    }
}

pub fn create_task(iface: GimbalInterface) -> anyhow::Result<GimbalTask> {
    let (evt_tx, evt_rx) = flume::bounded(256);

    Ok(EventTask {
        address: config.address,
        version: config.mavlink,
        evt_tx,
        evt_rx,
    })
}