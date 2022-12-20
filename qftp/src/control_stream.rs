use quinn::{SendStream, RecvStream};
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::io::{BufWriter, BufReader};
use paste::paste;

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

#[derive(Debug)]
pub struct ControlStream {
    send: BufWriter<SendStream>,
    recv: BufReader<RecvStream>
}

impl ControlStream {
    pub(crate) fn new(send: SendStream, recv: RecvStream) -> Self {
        let send = BufWriter::new(send);
        let recv = BufReader::new(recv);
        
        ControlStream { send, recv }
    }

    //add_read!{ u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64 }
    //add_write!{ u8 u16 u32 u64 u128 i8 i16 i32 i64 i128 f32 f64 }
}

