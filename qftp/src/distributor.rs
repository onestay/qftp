use crate::Error;
use quinn::{Connection, RecvStream};

use tokio::{sync::{oneshot::Sender, mpsc::UnboundedReceiver}, io::AsyncReadExt};

use tracing::{debug, trace};
use std::collections::HashMap;

#[derive(Debug)]
pub(crate) struct StreamRequest {
    num_streams: u16,
    request_id: u32,
    response: Vec<RecvStream>,
    response_sender: Sender<Vec<RecvStream>>,
}

impl StreamRequest {
    pub(crate) fn new(num_streams: u16, request_id: u32, sender: Sender<Vec<RecvStream>>) -> Self {
        let response = Vec::with_capacity(num_streams as usize);
        
        StreamRequest { num_streams, request_id, response, response_sender: sender }
    }

    pub(crate) fn is_done(&self) -> bool {
        self.response.len() == self.num_streams as usize
    }
}

pub(crate) async fn run(connection: Connection, mut channel: UnboundedReceiver<StreamRequest>) {
    trace!("starting the stream distributor");
    let mut messages = HashMap::new();
    let mut recv_stream_buffer = HashMap::new();

    // create a loop with a tokio::select! inside that checks connection.accept_{uni,bi} and channel.recv()
    loop {
        tokio::select! {        
            s = connection.accept_uni() => {
                let mut s = s.unwrap();
                let request_id = read_request_id(&mut s).await.unwrap();
                check_buffer(request_id, &mut messages, &mut recv_stream_buffer).await;
                match handle_stream(request_id, s, &mut messages, &mut recv_stream_buffer).await {
                    Some(request) => {
                        request.response_sender.send(request.response).unwrap();
                    },
                    None => continue
                };
            }

            m = channel.recv() => {
                trace!("got new message {m:?}");
                if let Some(m) = m {
                    messages.insert(m.request_id, m);
                } else {
                    debug!("accept_streams channel.recv returned none");
                    break;
                }

            }
        }
    }
}

async fn read_request_id(recv_stream: &mut RecvStream) -> Result<u32, Error> {
    let request_id = recv_stream.read_u32().await?;

    Ok(request_id)
}

async fn check_buffer(request_id: u32, messages: &mut HashMap<u32, StreamRequest>, recv_stream_buffer: &mut HashMap<u32, Vec<RecvStream>>) {
    // first check if we have the message
    trace!("checking if we recieved request {request_id}");
    if let Some(message) = messages.get_mut(&request_id) {
        // we have the message!
        // move the vec of recv_stream from the buffer to the message, if we have a buffer for the message
        trace!("we recieved the message");
        if let Some(mut buf) = recv_stream_buffer.remove(&request_id) {
            trace!("moving buffered streams to message");
            message.response.append(&mut buf);
            return;
        }
        trace!("we didn't have any buffered streams for the request")
    }
}

async fn handle_stream(request_id: u32, recv: RecvStream, messages: &mut HashMap<u32, StreamRequest>, recv_stream_buffer: &mut HashMap<u32, Vec<RecvStream>>) -> Option<StreamRequest> {
    
    trace!("read request_id {request_id} from the stream");
    if let Some(request) = messages.get_mut(&request_id) {
        trace!("found already existing request with {request_id}");
        request.response.push(recv);
        if request.is_done() {
            trace!("request with {request_id} is done");
            return Some(messages.remove(&request_id).unwrap());
        }
    } else if let Some(buf) = recv_stream_buffer.get_mut(&request_id) {
        trace!("couldn't find a request with id {request_id}, but found already existing buffer");
        buf.push(recv);
    } else {
        trace!("couldn't find a request with id {request_id}, creating new entry in buffer");
        let res = recv_stream_buffer.insert(request_id, vec![recv]);
        assert!(res.is_none());
    }
    
    None
}
