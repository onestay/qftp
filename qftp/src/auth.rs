use std::path::Path;

use crate::Error;
use serde::{Deserialize, Serialize};
use serde_json;
use thiserror::Error as ThisError;
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};

use argon2::{
    password_hash::{
        rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier,
        SaltString,
    },
    Argon2,
};

// TODO: add prevention of creating double users

#[derive(ThisError, Debug)]
pub enum AuthError {
    #[error("failed to find user")]
    UserNotFound,
    #[error("failed to validate password")]
    WrongPassword,
    #[error("password hash error")]
    PasswordHashError(password_hash::errors::Error),
}

impl From<password_hash::errors::Error> for AuthError {
    fn from(value: password_hash::errors::Error) -> Self {
        AuthError::PasswordHashError(value)
    }
}

#[async_trait::async_trait]
pub trait Storage {
    async fn add_user(&mut self, user: User) -> Result<(), Error>;
    async fn get_user<'a>(&mut self, name: &'a str) -> Result<User, Error> {
        let users = self.get_users().await?;
        match users.into_iter().find(|u| u.name == name) {
            Some(user) => Ok(user),
            None => Err(AuthError::UserNotFound.into()),
        }
    }
    async fn get_users(&mut self) -> Result<Vec<User>, Error>;
}

pub struct FileStorage {
    file: File,
}

impl FileStorage {
    pub async fn new(path: impl AsRef<Path>) -> Result<Self, Error> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .read(true)
            .open(path)
            .await?;

        Ok(FileStorage { file })
    }
}

#[async_trait::async_trait]
impl Storage for FileStorage {
    async fn add_user(&mut self, user: User) -> Result<(), Error> {
        let mut users = self.get_users().await?;
        users.push(user);

        let json = serde_json::to_string(&users)?;
        self.file.rewind().await?;
        self.file.write_all(json.as_bytes()).await?;
        self.file.flush().await?;
        Ok(())
    }

    async fn get_users(&mut self) -> Result<Vec<User>, Error> {
        let mut users = String::new();
        self.file.rewind().await?;
        self.file.read_to_string(&mut users).await?;
        match users.is_empty() {
            true => Ok(Vec::new()),
            false => Ok(serde_json::from_str(&users)?),
        }
    }
}

pub struct AuthManager<'key, T: Storage + Send> {
    storage: T,
    argon2: Argon2<'key>,
}

impl<'key, T: Storage + Send> AuthManager<'key, T> {
    pub fn new(storage: T) -> Self {
        AuthManager {
            storage,
            argon2: Argon2::default(),
        }
    }
}

impl<'key, T: Storage + Send> AuthManager<'key, T> {
    pub async fn add_user(
        &mut self,
        name: String,
        password: String,
        uid: u32,
        gid: Vec<u32>,
    ) -> Result<(), Error> {
        let salt = SaltString::generate(&mut OsRng);
        // TODO: remove this .unwrap()
        let password_hash = self
            .argon2
            .hash_password(password.as_bytes(), &salt)
            .unwrap()
            .to_string();
        let user = User {
            name,
            password: password_hash,
            uid,
            gid,
        };
        self.storage.add_user(user).await?;

        Ok(())
    }

    pub async fn get_user<'a>(
        &mut self,
        name: &'a str,
        password: &'a str,
    ) -> Result<User, Error> {
        let user = self.storage.get_user(name).await?;

        #[allow(clippy::redundant_closure)]
        let saved_password = PasswordHash::new(&user.password)
            .map_err(|e| Into::<AuthError>::into(e))?;

        match self
            .argon2
            .verify_password(password.as_bytes(), &saved_password)
        {
            Ok(()) => Ok(user),
            Err(password_hash::errors::Error::Password) => {
                Err(AuthError::WrongPassword.into())
            }
            Err(e) => {
                let e: AuthError = e.into();
                Err(e.into())
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    name: String,
    password: String,
    // GID and UID here refer to the GID and UID of the unix user
    // currently the whole auth system only works on unix
    uid: u32,
    gid: Vec<u32>,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::Error;
    #[tokio::test]
    async fn test_add_user() {
        let storage = FileStorage::new("./users.json")
            .await
            .expect("couldn't create FileStorage");
        let mut manager = AuthManager::new(storage);
        let result = manager
            .add_user(
                "test_user".to_string(),
                "test".to_string(),
                1001,
                vec![1001, 23, 523],
            )
            .await;
        assert!(result.is_ok());
        tokio::fs::remove_file("./users.json").await.unwrap();
    }

    #[tokio::test]
    async fn test_auth_user() {
        let storage = FileStorage::new("./test_auth_user.json")
            .await
            .expect("couldn't create FileStorage");
        let mut manager = AuthManager::new(storage);
        manager
            .add_user(
                "test_user".to_string(),
                "test".to_string(),
                1000,
                vec![1000],
            )
            .await
            .unwrap();
        let user = manager.get_user("test_user", "test").await;
        assert!(user.is_ok());
        let user = manager.get_user("test_user", "wrong_pass").await;
        assert!(
            matches!(user, Err(e) if matches!(e, Error::AuthenticationError(AuthError::WrongPassword)))
        );
        tokio::fs::remove_file("./test_auth_user.json")
            .await
            .unwrap();
    }
}
