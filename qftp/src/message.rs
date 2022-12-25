use std::fmt::Debug;

use qftp_derive::Message;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

use crate::Error;

#[async_trait::async_trait]
pub trait Message: Debug + Send {
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

#[derive(Message, Debug)]
pub struct Header {
    len: u8,
    message_ids: Vec<u8>,
}

impl Header {
    pub fn len(&self) -> u8 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn message_ids(&self) -> Vec<MessageType> {
        self.message_ids.iter().map(|id| (*id).into()).collect()
    }
}
pub enum MessageType {
    Version,
    VersionResponse,
    Login,
    LoginResponse,
    InvalidMessage,
}

#[derive(Message, Debug)]
pub struct Version {
    len: u8,
    versions: Vec<u8>,
}

impl Version {
    pub fn new(versions: &[u8]) -> Self {
        Version {
            len: versions.len() as u8,
            versions: Vec::from(versions),
        }
    }

    pub fn versions(&self) -> &Vec<u8> {
        &self.versions
    }
}

#[derive(Message, Debug)]
pub struct VersionResponse {
    negotiated_version: u8,
}

impl VersionResponse {
    pub fn new(version: u8) -> Self {
        VersionResponse { negotiated_version: version }
    }
}

impl From<u8> for MessageType {
    fn from(value: u8) -> Self {
        match value {
            0x00 => Self::Version,
            0x01 => Self::VersionResponse,
            0x02 => Self::Login,
            0x03 => Self::LoginResponse,
            _ => Self::InvalidMessage,
        }
    }
}

#[cfg(test)]
mod test {
    use super::Message;
    use super::Version;
    #[test]
    fn test_version() {
        let v = Version {
            len: 2,
            versions: vec![1, 2],
        };

        assert_eq!([2, 1, 2], v.to_bytes().as_slice())
    }
}
