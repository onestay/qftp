use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};
use thiserror::Error as ThisError;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tracing::{error, trace};

#[derive(Debug, ThisError)]
pub enum FileError {
    #[error("the base path has to be a directory or don't have permissions")]
    BasePathNotADir,
    #[error("used an absolute path as an argument to a function wanting an offset")]
    PathIsAbsolute,
    #[error("IO error occured")]
    IOError(#[from] std::io::Error),
    #[error("failed to convert OsString to String")]
    OsStringConversionError,
    #[error("join error")]
    JoinError(#[from] tokio::task::JoinError),
}

// TODO: the usage of Path/PathBuf/impl AsRef<Path> is all over the place in this module
// There is definitely a good way of doing all this way more efficient without all the
// allocations.

// TODO: currently everything just returns a Vec<T> of sorts. This is pretty inefficient.
// At some point all of these could be changed to streaming bytes/files/whatever is being returned

#[derive(Debug)]
pub struct FileManager {
    base_path: PathBuf,
}

// TODO: does path and relative path both need to be PathBuf?
// There is probably some slice stuff possible here
// Maybe just storing the number of parts that are offset and then on the fly computing the relative path?
#[derive(Debug)]
pub struct QFile {
    pub(crate) metadata: Metadata,
    pub(crate) path: PathBuf,
    pub(crate) relative_path: PathBuf,
    file: Option<File>,
}

impl QFile {
    pub fn new(metadata: Metadata, path: PathBuf, relative_path: PathBuf) -> Self {
        QFile {
            metadata,
            path,
            relative_path,
            file: None,
        }
    }

    pub fn file(&mut self) -> Result<&mut File, FileError> {
        let file = File::open(&self.path)?;

        self.file = Some(file);
        Ok(self.file.as_mut().unwrap())
    }

    pub async fn send<T>(&mut self, writer: &mut T) -> Result<(), FileError>
    where
        T: AsyncWrite + Send + Sync + Unpin,
    {
        let mut len = self.metadata.len() as usize;
        let fs_file = self.file()?;
        let mut buf = [0; 4096];

        while len != 0 {
            let mut read = len;
            match len {
                _ if len <= 4096 => {
                    trace!("len: {len}, reading remaining bytes");
                    fs_file.read_exact(&mut buf[0..len]).unwrap();
                    len -= len;
                }
                _ => {
                    trace!("len: {len}, reading whole buffer");
                    fs_file.read_exact(&mut buf).unwrap();
                    len -= 4096;
                }
            }

            read -= len;

            writer.write_all(&buf[0..read]).await?;
            trace!("Wrote {read} bytes. {len} bytes remaining");
        }
        trace!("finish writing file");
        Ok(())
    }
}

impl FileManager {
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self, FileError> {
        let mut base_path_buf = PathBuf::new();
        base_path_buf.push(base_path);
        base_path_buf.canonicalize()?;
        if !base_path_buf.is_dir() {
            return Err(FileError::BasePathNotADir);
        }

        Ok(FileManager {
            base_path: base_path_buf,
        })
    }

    fn walk_dir_impl(
        path: impl AsRef<Path>,
        offset: impl AsRef<Path> + Copy,
        result: &mut Vec<QFile>,
    ) -> Result<(), FileError> {
        let dir = fs::read_dir(&path)?;

        for entry in dir {
            let entry = entry?;

            let file_type = entry.file_type()?;

            if file_type.is_dir() {
                let mut offset = offset.as_ref().to_path_buf();
                offset.push(entry.path().iter().last().unwrap());

                FileManager::walk_dir_impl(entry.path(), &offset, result)?;
            } else if file_type.is_file() {
                // TODO: there are a lot of allocations here
                // There is definitely a more efficient way to do this.
                let mut relative_path = PathBuf::new();
                relative_path.push(offset);
                relative_path.push(entry.file_name());

                let mut full_path = PathBuf::new();
                full_path.push(path.as_ref());
                full_path.push(entry.file_name());
                result.push(QFile::new(
                    entry.metadata().unwrap(),
                    full_path,
                    relative_path,
                ));
            }
        }

        Ok(())
    }

    pub(crate) async fn walk_dir(
        &self,
        offset: impl AsRef<Path> + Send + 'static + Copy,
    ) -> Result<Vec<QFile>, FileError> {
        let mut base_path = self.base_path.clone();
        if !offset.as_ref().as_os_str().is_empty() {
            if offset.as_ref().is_absolute() {
                return Err(FileError::PathIsAbsolute);
            }

            base_path.push(offset);
        }
        let result: Result<Vec<QFile>, FileError> = tokio::task::spawn_blocking(move || {
            let mut result = Vec::new();
            FileManager::walk_dir_impl(base_path, offset, &mut result)?;

            Ok(result)
        })
        .await?;

        result
    }
    // this is a first implementation of this leaving a lot of performance on the table
    // eventually I want to use some io_uring magic to make reading a lot of files really fast
    // this will also read all the files into memory first
    pub(crate) async fn _read_file(
        &self,
        _offset: impl AsRef<Path> + Send + 'static + Copy,
    ) -> Result<Vec<u8>, FileError> {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use super::FileManager;
    #[tokio::test]
    async fn test_walk_dir() {
        let path = format!("{}/tests/walk_dir", env!("CARGO_MANIFEST_DIR"));
        println!("{path}");
        let f = FileManager::new(path).expect("expect creating a file manager not to fail");
        let result = f.walk_dir("").await.unwrap();
        println!("{result:#?}");
        assert_eq!(result.len(), 4)
    }

    #[tokio::test]
    async fn test_walk_dir_with_offset() {
        let path = format!("{}/tests/walk_dir", env!("CARGO_MANIFEST_DIR"));
        println!("{path}");
        let f = FileManager::new(path).expect("expect creating a file manager not to fail");
        let result = f.walk_dir("b").await.unwrap();

        assert_eq!(result.len(), 2)
    }
}
