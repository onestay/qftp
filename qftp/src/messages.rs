use qftp_derive::Message;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use crate::ControlStream;
use crate::Error;

#[async_trait::async_trait]
pub trait Message {
    async fn recv<T>(reader: &mut T) -> Result<Self, Error>
    where
        Self: Sized,
        T: Sync + Send + Unpin + AsyncRead;

    fn to_bytes(self) -> Vec<u8>;

    async fn send<T>(self, writer: &mut T) -> Result<(), Error>
    where
        Self: Sized,
        T: Sync + Send + Unpin + AsyncWrite,
    {
        let b = self.to_bytes();
        writer.write_all(&b).await?;
        Ok(())
    }
}

pub trait SayHello {
    fn hello();
}

pub enum MessageType {
    Version,
    VersionResponse,
    Login,
    LoginResponse,
}

#[derive(Message)]
pub struct Version {
    len: u8,
    versions: Vec<u8>,
}

#[derive(Message)]
pub struct TestDerive {
    len: u8,
    swag: u64,
    ur_mom: f64,
    asd: Vec<i32>,
    cool: i128,
}

pub struct VersionResponse {
    negotiated_version: u8,
}

impl TryFrom<u8> for MessageType {
    type Error = Error;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(Self::Version),
            0x01 => Ok(Self::VersionResponse),
            0x02 => Ok(Self::Login),
            0x03 => Ok(Self::LoginResponse),
            _ => Err(Error::MessageIDError(value)),
        }
    }
}

#[cfg(test)]
mod test {
    use super::Message;
    use super::TestDerive;
    use super::Version;
    #[test]
    fn test_version() {
        let v = Version {
            len: 2,
            versions: vec![1,2],
        };

        assert_eq!([2,1,2], v.to_bytes().as_slice())
    }

    #[test]
    fn test() {
        let v = TestDerive {
            len: 254,
            swag: 35215,
            ur_mom: 124.12625,
            asd: vec![214,2351,2,41,5,23],
            cool: -12321
        };

        println!("{:x?}", v.to_bytes());
    }
}
