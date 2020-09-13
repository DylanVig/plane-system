use std::future::Future;

use smol::channel::{Receiver, Sender};

pub mod camera;
pub mod gimbal;
pub mod pixhawk;

#[derive(Debug, Clone)]
pub struct Channels<Request: Send, Response: Send> {
    request_channel: (Sender<Request>, Receiver<Request>),
    response_channel: (Sender<Response>, Receiver<Response>),
}

impl<Request: Send, Response: Send> Channels<Request, Response> {
    pub fn new() -> Self {
        Channels {
            request_channel: smol::channel::unbounded(),
            response_channel: smol::channel::unbounded(),
        }
    }

    pub fn send_request(
        &self,
        request: Request,
    ) -> impl Future<Output = Result<(), smol::channel::SendError<Request>>> + '_ {
        self.request_channel.0.send(request)
    }

    pub fn send_response(
        &self,
        response: Response,
    ) -> impl Future<Output = Result<(), smol::channel::SendError<Response>>> + '_ {
        self.response_channel.0.send(response)
    }

    pub fn recv_request(
        &self,
    ) -> impl Future<Output = Result<Request, smol::channel::RecvError>> + '_ {
        self.request_channel.1.recv()
    }

    pub fn recv_response(
        &self,
    ) -> impl Future<Output = Result<Response, smol::channel::RecvError>> + '_ {
        self.response_channel.1.recv()
    }
}
