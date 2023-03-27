//file to control processing commands and then calling plane system modes and interacting between them
use crossterm::event::{read, Event, KeyCode, KeyEvent, KeyModifiers};

use tokio::time;


pub enum Modes {
    Search,
    Standby,
    None,
}



pub struct ControlTask {
    cmd_rx: ChannelCommandSource<SearchRequest, SearchResponse>,
    cmd_tx: ChannelCommandSink<SearchRequest, SearchResponse>,
    camera_ctrl_cmd_tx: flume::Sender<CameraRequest>,
    telem_rx: watch::Receiver<Telemetry>,
}

impl ControlTask {
    pub(super) fn new(camera_ctrl_cmd_tx: flume::Sender<CameraRequest>,
        telem_rx: watch::Receiver<Telemetry>) -> Self {
        let (cmd_tx, cmd_rx) = flume::bounded(256);



        Self {
            camera_cntrl_cmd_tx,
            telem_rx,
            cmd_rx,
            cmd_tx,
        }
    }

    pub fn cmd(&self) -> ChannelCommandSink<SearchRequest, SearchResponse> {
        self.cmd_tx.clone()
    }

}

async fn timedSearch(active: u16, inactive: u16) {
    
    loop {
        //TODO: call continous capture
        sleep(active * 1000); //s to ms
                              //TODO: exit continous capture
        sleep(inactive * 1000);
    }
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
                                {
                                    todo!()
                                }
                                SearchRequest::Distance(distance, waypoint) => todo!(),
                                SearchRequest::Manual() =>
                                //wait for buttonpress

                                //run until the buttonpress happens again
                                // want to race a wait for a button press with continuis capture

                                //redo
                                ControlTask
                                {
                                    todo!()
                                }
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
