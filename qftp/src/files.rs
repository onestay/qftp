use crate::message;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum FileError {
    #[error("the base path has to be a directory or don't have permissions")]
    BasePathNotADir,
    #[error(
        "used an absolute path as an argument to a function wanting an offset"
    )]
    PathIsAbsolute,
    #[error("IO error occured")]
    IOError(#[from] std::io::Error),
    #[error("failed to convert OsString to String")]
    OsStringConversionError,
    #[error("join error")]
    JoinError(#[from] tokio::task::JoinError),
}

#[derive(Debug)]
pub struct FileManager {
    base_path: PathBuf,
}

impl FileManager {
    pub fn new(base_path: impl AsRef<Path>) -> Result<Self, FileError> {
        let mut base_path_buf = PathBuf::new();
        base_path_buf.push(base_path);
        if !base_path_buf.is_dir() {
            return Err(FileError::BasePathNotADir);
        }

        Ok(FileManager {
            base_path: base_path_buf,
        })
    }

    fn walk_dir_impl(
        path: impl AsRef<Path>,
    ) -> Result<Vec<message::ListFileResponse>, FileError> {
        let mut result = Vec::new();
        let dir = fs::read_dir(path)?;

        for entry in dir {
            let entry = entry?;
            let file_type = entry.file_type()?;

            if file_type.is_dir() {
                let mut recurse_result =
                    FileManager::walk_dir_impl(entry.path())?;
                result.append(&mut recurse_result);
            } else if file_type.is_file() {
                let path = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| FileError::OsStringConversionError)?;
                result.push(message::ListFileResponse::new(
                    path,
                    &entry.metadata()?,
                ))
            }
        }

        Ok(result)
    }

    pub(crate) async fn walk_dir(
        &self,
        offset: Option<impl AsRef<Path>>,
    ) -> Result<Vec<message::ListFileResponse>, FileError> {
        let mut base_path = self.base_path.clone();
        if let Some(offset) = offset {
            if offset.as_ref().is_absolute() {
                return Err(FileError::PathIsAbsolute);
            }

            base_path.push(offset);
        }
        let result = tokio::task::spawn_blocking(move || {
            FileManager::walk_dir_impl(base_path)
        })
        .await?;

        result
    }
}

#[cfg(test)]
mod test {
    use super::FileManager;
    #[tokio::test]
    async fn test_walk_dir() {
        let f = FileManager::new("/Users/marius/Documents/dev/rust/qftp/qftp")
            .expect("expect creating a file manager not to fail");
        let result = f.walk_dir(None::<&str>).await.unwrap();

        println!("{result:#?}")
    }
}
