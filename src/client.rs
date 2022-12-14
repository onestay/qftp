use crate::Error;
use quinn::{Connection, Endpoint, RecvStream, SendStream};
use rustls::{
    client::{ServerCertVerified, ServerCertVerifier},
    Certificate, ClientConfig, RootCertStore,
};

use tracing::{debug, error, info, span, warn, Level};

use crate::messages::{HelloMessage, Sendable};
use std::{net::SocketAddr, sync::Arc};
#[derive(Debug)]
pub struct Client {
    connection: Connection,
    control_stream: (SendStream, RecvStream),
}

impl Client {
    pub async fn new(addr: SocketAddr) -> Result<Self, Error> {
        debug!("Creating client config");
        let client_config = ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(Arc::new(DontVerify {}))
            .with_no_client_auth();
        debug!("Creating client");
        let mut client = Endpoint::client(
            "0.0.0.0:0".parse().expect("Failed to parse address"),
        )?;
        client.set_default_client_config(quinn::ClientConfig::new(Arc::new(
            client_config,
        )));
        debug!("Connecting to server");
        let connection = client.connect(addr, "test.server")?.await?;

        debug!("Opening control_stream");
        let control_stream = connection.open_bi().await?;

        let mut client = Client {
            connection,
            control_stream,
        };

        client.negotiate_version().await?;

        Ok(client)
    }
    async fn negotiate_version(&mut self) -> Result<(), Error> {
        let negotation_message = HelloMessage::default().to_bytes();
        debug!("Sending version negotiation message: {:?}", negotation_message);
        self.control_stream.0.write_all(&negotation_message).await?;
        let mut buf: [u8; 2] = [0; 2];
        self.control_stream.1.read_exact(&mut buf).await.unwrap();
        Ok(())
    }
}

struct DontVerify;

impl ServerCertVerifier for DontVerify {
    fn verify_server_cert(
        &self,
        _: &Certificate,
        _: &[Certificate],
        _: &rustls::ServerName,
        _: &mut dyn Iterator<Item = &[u8]>,
        _: &[u8],
        _: std::time::SystemTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
}
