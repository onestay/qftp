use crate::Error;
use quinn::Endpoint;
use rustls::ServerConfig;
use rustls::{Certificate, PrivateKey};
use std::net::SocketAddr;
use std::sync::Arc;

use tracing::debug;

use crate::connected_client::ConnectedClient;

/// Entrypoint for creating a qftp Server
#[derive(Debug)]
pub struct Server {
    endpoint: Endpoint,
}

impl Server {
    fn create_endpoint(
        listen_addr: SocketAddr,
        cert: Certificate,
        priv_key: PrivateKey,
    ) -> Result<Endpoint, Error> {
        let server_config = ServerConfig::builder()
            .with_safe_defaults()
            .with_no_client_auth()
            .with_single_cert(vec![cert], priv_key)?;
        let server_config =
            quinn::ServerConfig::with_crypto(Arc::new(server_config));
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
    pub fn new(
        listen_addr: SocketAddr,
        cert: Certificate,
        priv_key: PrivateKey,
    ) -> Result<Self, Error> {
        let server = Server::create_endpoint(listen_addr, cert, priv_key)?;
        Ok(Server { endpoint: server })
    }

    /// Accepts a connecting qftp client
    pub async fn accept(&self) -> Result<ConnectedClient, Error> {
        loop {
            if let Some(connection) = self.endpoint.accept().await {
                let connection = connection.await?;
                debug!("accepted a new client");
                return ConnectedClient::new(connection).await;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;

    fn read_test_certs() -> (Certificate, PrivateKey) {
        let cert =
            fs::read("cert/miu.local.crt").expect("Failed to read certificate");
        let cert = Certificate(cert);
        let priv_key =
            fs::read("cert/miu.local.der").expect("Failed to read private key");
        let priv_key = PrivateKey(priv_key);

        (cert, priv_key)
    }

    #[tokio::test]
    async fn test_server_new_valid() {
        let (cert, priv_key) = read_test_certs();
        let server = Server::new(
            "0.0.0.0:0".parse().expect("Failed to parse socket addr"),
            cert,
            priv_key,
        );
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_server_invalid_priv_key_cert() {
        let cert = Certificate(vec![0]);
        let priv_key = PrivateKey(vec![0]);
        let server = Server::new(
            "0.0.0.0:0".parse().expect("Failed to parse socket addr"),
            cert,
            priv_key,
        );
        assert!(server.is_err());
    }
}