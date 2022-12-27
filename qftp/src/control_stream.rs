use quinn::{SendStream, RecvStream};
use tracing::trace;
use crate::Error;
use crate::message::{self, Message};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use paste::paste;
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

    

    pub async fn recv_message<T: Message + Send>(&mut self) -> Result<T, Error> {
        trace!("recieving message {:?}", std::any::type_name::<T>());
        let result = T::recv(self.recv()).await?;
        trace!("recieved {:?}", result);

        Ok(result)
    }

    pub async fn next_message_id(&mut self) -> Result<message::MessageType, Error> {
        let id = self.read_u8().await?;
        match id.into() {
            message::MessageType::InvalidMessage => {
                Err(Error::MessageIDError(id))
            },
            message_type => Ok(message_type)
        }
    }

    add_read!{ u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64 }
    add_write!{ u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64 }
}

