//file to control processing commands and then calling plane system modes and itneracting between them

pub enum Modes {
    Search,
    Standby,
    None,
}

pub struct ControlTask {
    ctrl_evt_rx: flume::Receiver<ControlEvent>,
    ctrl_evt_tx: flume::Sender<ControlEvent>,
    cmd_rx: ChannelCommandSource<CameraRequest, CameraResponse>,
    cmd_tx: ChannelCommandSink<CameraRequest, CameraResponse>,
}

impl ControlTask {
    pub(super) fn new() -> Self {
        let (cmd_tx, cmd_rx) = flume::bounded(256);
        let (ctrl_evt_tx, ctrl_evt_rx) = flume::bounded(256);

        Self {
            ctrl_evt_rx,
            ctrl_evt_tx,
            cmd_rx,
            cmd_tx,
        }
    }

    pub fn cmd(&self) -> ChannelCommandSink<CameraRequest, CameraResponse> {
        self.cmd_tx.clone()
    }

    pub fn event(&self) -> flume::Receiver<ControlEvent> {
        self.ctrl_evt_rx.clone()
    }
}

async fn timedSearch(active: u16, u16 inactive) {
    
}



#[async_trait]
impl Task for ControlTask {
    fn name(&self) -> &'static str {
        "modes/control"
    }

    async fn run(self: Box<Self>, cancel: CancellationToken) -> anyhow::Result<()> {
        let loop_fut = async move {
            let ctrl_evt_tx = self.ctrl_evt_tx;
            loop {
                match self.cmd_rx.recv_async().await {
                    Ok((req, ret)) => {
                        let result = match req {
                            ModeRequest::Inactive(_) => todo!(),
                            ModeRequest::ZoomControl(req) => todo!(),
                            ModeRequest::Search(req) => match req {
                                //Use .select with within distance to cancel search when out of range
                                SearchRequest::Time(active, inactive) => 
                                //standby mode

                                //wait for sometime

                                //searchmode

                                //figure out how to call cc 

                                //time 

                                //ad nauseaum
                                
                                
                                todo!(),
                                SearchRequest::Distance(distance, waypoint) => todo!(),
                                SearchRequest::Manual() => 
                                //wait for buttonpress
                                
                                //run until the buttonpress works
                                
                                
                                todo!(),
                            },
                        };

                        let _ = ret.send(result);
                    }
                    Err(_) => break,
                }
            }

            Ok::<_, anyhow::Error>(())
        };

        select! {
          _ = cancel.cancelled() => {}
          res = loop_fut => { res? }
        }

        Ok(())
    }


}
