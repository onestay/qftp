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
        result: &mut Vec<message::ListFileResponse>,
    ) -> Result<(), FileError> {
        let dir = fs::read_dir(path)?;

        for entry in dir {
            let entry = entry?;

            let file_type = entry.file_type()?;

            if file_type.is_dir() {
                let mut offset = offset.as_ref().to_path_buf();
                offset.push(entry.path().iter().last().unwrap());

                FileManager::walk_dir_impl(entry.path(), &offset, result)?;
            } else if file_type.is_file() {
                let mut relative_path = PathBuf::new();
                relative_path.push(offset);
                relative_path.push(entry.file_name());

                let path = relative_path
                    .into_os_string()
                    .into_string()
                    .map_err(|_| FileError::OsStringConversionError)?;

                result.push(message::ListFileResponse::new(
                    path,
                    &entry.metadata()?,
                ))
            }
        }

        Ok(())
    }

    pub(crate) async fn walk_dir(
        &self,
        offset: impl AsRef<Path> + Send + 'static + Copy,
    ) -> Result<Vec<message::ListFileResponse>, FileError> {
        let mut base_path = self.base_path.clone();
        if !offset.as_ref().as_os_str().is_empty() {
            if offset.as_ref().is_absolute() {
                return Err(FileError::PathIsAbsolute);
            }

            base_path.push(offset);
        }
        let result: Result<Vec<message::ListFileResponse>, FileError> =
            tokio::task::spawn_blocking(move || {
                let mut result = Vec::new();
                FileManager::walk_dir_impl(base_path, offset, &mut result)?;

                Ok(result)
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
        let path = format!("{}/tests/walk_dir", env!("CARGO_MANIFEST_DIR"));
        println!("{path}");
        let f = FileManager::new(path)
            .expect("expect creating a file manager not to fail");
        let result = f.walk_dir("").await.unwrap();

        assert_eq!(result.len(), 4)
    }

    #[tokio::test]
    async fn test_walk_dir_with_offset() {
        let path = format!("{}/tests/walk_dir", env!("CARGO_MANIFEST_DIR"));
        println!("{path}");
        let f = FileManager::new(path)
            .expect("expect creating a file manager not to fail");
        let result = f.walk_dir("b").await.unwrap();

        assert_eq!(result.len(), 2)
    }
}
