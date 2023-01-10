use crate::message::Message;
use crate::Error;
use quinn::{RecvStream, SendStream};
use tracing::trace;

// these macros are currently unused, but maybe I'll use them at some point
// macro_rules! add_read {
//     ($($t:ty)*) => ($(
//         paste! {
//             pub(crate) async fn [<read_ $t>](&mut self) -> Result<$t, crate::Error> {
//                 let n = self.recv.[<read_ $t>]().await?;
//                 Ok(n)
//             }
//         }
//     )*)
// }

// macro_rules! add_write {
//     ($($t:ty)*) => ($(
//         paste! {
//             pub(crate) async fn [<write_ $t>](&mut self, n: $t) -> Result<(), crate::Error> {
//                 self.send.[<write_ $t>](n).await?;
//                 Ok(())
//             }
//         }
//     )*)
// }

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

    pub async fn send_message<T: Message + Send>(
        &mut self,
        message: T,
    ) -> Result<(), Error> {
        trace!("sending message: {:#?}", message);
        message.send(&mut self.send).await?;
        Ok(())
    }

    pub async fn recv_message<T: Message + Send>(&mut self) -> Result<T, Error> {
        trace!("recieving message {:#?}", std::any::type_name::<T>());
        let result = T::recv(self.recv()).await?;
        trace!("recieved {:?}", result);

        Ok(result)
    }
}
