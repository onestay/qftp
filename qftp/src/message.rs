use std::{
    fmt::{self, Debug},
    fs::Metadata,
    os::unix::fs::MetadataExt,
    time::{Duration, SystemTime},
};

use qftp_derive::Message;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, AsyncReadExt};

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

pub(crate) enum Request {
    ListFileRequest(ListFilesRequest),
}

impl Request {
    pub(crate) async fn next_request<T>(reader: &mut T) -> Result<Self, Error>
    where
        Self: Sized,
        T: Sync + Send + Unpin + AsyncRead,
    {
        let request_id = reader.read_u16().await?;
        match request_id {
            0x1 => {
                let request = ListFilesRequest::recv(reader).await?;

                Ok(Self::ListFileRequest(request))
            },
            id => {
                Err(Error::MessageIDError(id))
            }
            
        }
    }
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
    pub negotiated_version: u8,
}

impl VersionResponse {
    pub fn new(version: u8) -> Self {
        VersionResponse {
            negotiated_version: version,
        }
    }
}

#[derive(Message, Debug)]
pub struct LoginRequest {
    name_length: u8,
    name: String,
    password_length: u8,
    password: String,
}

impl LoginRequest {
    /// Create a new LoginRequest
    ///
    /// # Panic
    /// This function panics if the length of name or password is longer than u8::MAX
    pub fn new(name: String, password: String) -> Self {
        if name.len() > u8::MAX.into() || password.len() > u8::MAX.into() {
            panic!("`name` or `password` are longer than {}", u8::MAX);
        }
        LoginRequest {
            name_length: name.len() as u8,
            name,
            password_length: password.len() as u8,
            password,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn password(&self) -> &str {
        &self.password
    }
}

/// Response to the [LoginRequest](crate::message::LoginRequest)
#[derive(Message, Debug)]
pub struct LoginResponse {
    status: u8,
}

impl LoginResponse {
    pub fn is_ok(&self) -> bool {
        self.status != 0
    }

    pub fn new(is_ok: bool) -> Self {
        LoginResponse {
            status: is_ok as u8,
        }
    }
}

#[derive(Debug, Message)]
pub struct ListFilesRequest {
    path_len: u32,
    path: String,
    request_id: u32,
}

impl ListFilesRequest {
    pub(crate) fn new(path: String) -> ListFilesRequest {
        ListFilesRequest {
            path_len: path.len() as u32,
            path,
            request_id: 1325,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn request_id(&self) -> u32 {
        self.request_id
    }
}

#[derive(Debug, Message)]
pub struct ListFileResponseHeader {
    pub num_files: u32,
}

impl ListFileResponseHeader {
    pub fn num_files(&self) -> u32 {
        self.num_files
    }
}

#[derive(Debug, Message)]
pub struct ListFileResponse {
    file_name_length: u32,
    file_name: String,
    file_len: u64,
    accessed: i64,
    created: i64,
    modified: i64,
    mode: u32,
}

impl fmt::Display for ListFileResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n\tSize: {} bytes\n\tAccessed: {:?}\n\tCreated: {:?}\n\tModified: {:?}", 
        self.file_name, self.file_len, self.accessed(), self.created(), self.modified())
    }
}

impl ListFileResponse {
    pub fn new(file_name: impl ToString, metadata: &Metadata) -> Self {
        let file_name = file_name.to_string();
        ListFileResponse {
            file_name_length: file_name.len() as u32,
            file_name,
            file_len: metadata.size(),
            accessed: metadata.atime(),
            created: metadata.ctime(),
            modified: metadata.mtime(),
            mode: metadata.mode(),
        }
    }

    pub fn file_name(&self) -> &str {
        &self.file_name
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> u64 {
        self.file_len
    }

    pub fn accessed(&self) -> SystemTime {
        let duration = Duration::from_millis(self.accessed as u64);

        SystemTime::UNIX_EPOCH + duration
    }

    pub fn created(&self) -> SystemTime {
        let duration = Duration::from_millis(self.created as u64);

        SystemTime::UNIX_EPOCH + duration
    }

    pub fn modified(&self) -> SystemTime {
        let duration = Duration::from_millis(self.modified as u64);

        SystemTime::UNIX_EPOCH + duration
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_version() {
        let v = Version {
            len: 2,
            versions: vec![1, 2],
        };

        assert_eq!([2, 1, 2], v.to_bytes().as_slice())
    }

    #[test]
    fn test_login() {
        let login = LoginRequest {
            name_length: 5,
            name: "12345".to_string(),
            password_length: 2,
            password: "ab".to_string(),
        };

        assert_eq!(
            [0, 5, 0x31, 0x32, 0x33, 0x34, 0x35, 0, 2, 0x61, 0x62],
            login.to_bytes().as_slice()
        );
    }
}
