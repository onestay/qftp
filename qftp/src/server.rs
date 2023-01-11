use crate::auth::{AuthManager, FileStorage};
use crate::Error;
use quinn::Endpoint;
use rustls::ServerConfig;
use rustls::{Certificate, PrivateKey};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;

use crate::connected_client::ConnectedClient;
use crate::files::FileManager;

#[derive(Debug)]
pub struct ServerBuilder {
    server_config: Option<ServerConfig>,
    listen_addr: Option<SocketAddr>,
    base_path: Option<PathBuf>,
    auth_file: Option<PathBuf>,
}

impl ServerBuilder {
    /// set a custom [ServerConfig](rustls::ServerConfig)
    pub fn with_server_config(mut self, config: ServerConfig) -> Self {
        self.server_config = Some(config);

        self
    }

    /// set the address to listen on
    pub fn set_listen_addr(mut self, addr: SocketAddr) -> Self {
        self.listen_addr = Some(addr);

        self
    }

    /// set the base path for serving files
    pub fn set_base_path(mut self, path: PathBuf) -> Self {
        if !path.is_dir() {
            panic!("base path has to be a directory");
        }

        self.base_path = Some(path);

        self
    }

    /// set the location for the client authentication file
    pub fn set_auth_file(mut self, auth_file: PathBuf) -> Self {
        if !auth_file.is_file() {
            panic!("auth_file has to be a path to a file");
        }

        self.auth_file = Some(auth_file);

        self
    }

    /// Creates a new default [ServerConfig](rustls::ServerConfig) with the specified certs.
    /// If you want to supply your own server config you can use [with_server_config](ServerBuilder::with_server_config)
    pub fn with_certs(mut self, certs: Vec<Certificate>, private_key: PrivateKey) -> Self {
        let server_config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(certs, private_key)
            .expect("Couldn't construct new ServerConfig");

        self.server_config = Some(server_config);

        self
    }

    pub async fn build(self) -> Result<Server, Error> {
        let server = Server::new(
            self.listen_addr.expect("didn't set listen_addr"),
            self.server_config.expect("didn't set ServerConfig"),
            self.auth_file.expect("didn't set auth_file"),
            self.base_path.expect("didn't set base_path"),
        )
        .await?;

        Ok(server)
    }
}

/// Entrypoint for creating a qftp Server
#[derive(Debug)]
pub struct Server {
    endpoint: Endpoint,
    auth: Arc<Mutex<AuthManager<FileStorage>>>,
    file_manager: Arc<FileManager>,
}

impl Server {
    pub fn builder() -> ServerBuilder {
        ServerBuilder {
            server_config: None,
            listen_addr: None,
            base_path: None,
            auth_file: None,
        }
    }

    fn create_endpoint(
        listen_addr: SocketAddr,
        server_config: ServerConfig,
    ) -> Result<Endpoint, Error> {
        let server_config = quinn::ServerConfig::with_crypto(Arc::new(server_config));
        let server = Endpoint::server(server_config, listen_addr)?;
        Ok(server)
    }

    /// Creates a new `Server` listening on the given addr
    ///
    /// # Arguments
    ///
    /// * `listen_addr` - The addr to listen on
    /// * `cert` - The certificate to present to a connecting client. Refer to [rustls](rustls::Certificate) documentation for the correct format
    /// * `priv_key` - The private key. Refer to [rustls](rustls::PrivateKey) documentation for the correct format
    pub async fn new(
        listen_addr: SocketAddr,
        server_config: ServerConfig,
        auth_file: PathBuf,
        base_path: PathBuf,
    ) -> Result<Self, Error> {
        let server = Server::create_endpoint(listen_addr, server_config)?;
        let auth_storage = FileStorage::new(auth_file).await?;
        let manager = AuthManager::new(auth_storage);
        let file_manager = FileManager::new(base_path).unwrap();
        Ok(Server {
            endpoint: server,
            auth: Arc::new(Mutex::new(manager)),
            file_manager: Arc::new(file_manager),
        })
    }

    /// Accepts a connecting qftp client
    pub async fn accept(&self) -> Result<ConnectedClient, Error> {
        loop {
            if let Some(connection) = self.endpoint.accept().await {
                let connection = connection.await?;
                debug!("accepted a new client");
                return ConnectedClient::new(
                    connection,
                    self.auth.clone(),
                    self.file_manager.clone(),
                )
                .await;
            }
        }
    }
}
