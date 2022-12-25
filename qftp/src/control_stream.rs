use quinn::{SendStream, RecvStream};
use tracing::trace;
use crate::Error;
use crate::message::{self, Message};

#[derive(Debug)]
enum State {
    Start,
    VersionNegotationDone,
    Unauthenticated,
    Authenciated,
    RecvHeader,
    RecvMessage,
}

macro_rules! add_read {
    ($($t:ty)*) => ($(
        paste! {
            pub(crate) async fn [<read_ $t>](&mut self) -> Result<$t, crate::Error> {
                let n = self.recv.[<read_ $t>]().await?;
                Ok(n)
            }
        }
    )*)
}

macro_rules! add_write {
    ($($t:ty)*) => ($(
        paste! {
            pub(crate) async fn [<write_ $t>](&mut self, n: $t) -> Result<(), crate::Error> {
                self.send.[<write_ $t>](n).await?;
                Ok(())
            }
        }
    )*)
}

/// The ControlStream struct exchanges control messages between the Server and a Client
// TODO: Maybe make send and recv generic with AsyncRead and AsyncWrite
#[derive(Debug)]
pub struct ControlStream {
    send: SendStream,
    recv: RecvStream,
}

impl ControlStream {
    pub(crate) fn new(send: SendStream, recv: RecvStream) -> Self {
        trace!("creating new ControlStream");
        
        ControlStream { send, recv }
    }

    pub(crate) fn recv(&mut self) -> &mut RecvStream {
        &mut self.recv
    }

    pub(crate) fn send(&mut self) -> &mut SendStream {
        &mut self.send
    }

    pub async fn send_message<T: Message + Send>(&mut self, message: T) -> Result<(), Error> {
        trace!("sending message: {:?}", message);
        message.send(&mut self.send).await?;
        Ok(())
    }

    // Not really the biggest fan of the dynamic dispatch but not sure what else could be done here
    pub async fn recv_messages(&mut self) -> Result<Box<dyn Message>, Error> {
        let header = message::Header::recv(&mut self.recv).await?;
        for id in header.message_ids() {
            match id {
                _ => todo!()
            }
        }
        todo!()
    }

    //add_read!{ u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64 }
    //add_write!{ u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64 }
}

