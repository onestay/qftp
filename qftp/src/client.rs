use crate::{Error, message::Message};
use quinn::{Connection, Endpoint, RecvStream, SendStream};
use rustls::{
    client::{ServerCertVerified, ServerCertVerifier},
    Certificate, ClientConfig, RootCertStore,
};

use tracing::{debug, error, info, span, warn, Level, trace};
use crate::ControlStream;
use std::{net::SocketAddr, sync::Arc};
use crate::message;

/// Entrypoint for creating a qftp Client
#[derive(Debug)]
pub struct Client {
    connection: Connection,
    control_stream: ControlStream,
}

impl Client {
    fn create_endpoint() -> Result<Endpoint, Error> {
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

        Ok(client)
    }
    pub async fn new(addr: SocketAddr) -> Result<Self, Error> {
        let client = Client::create_endpoint()?;
        debug!("Connecting to server");
        let connection = client.connect(addr, "test.server")?.await?;

        debug!("Opening control_stream");
        let control_stream = connection.open_bi().await?;
        let control_stream = ControlStream::new(control_stream.0, control_stream.1);
        let mut client = Client {
            connection,
            control_stream,
        };

        client.negotiate_version().await?;

        Ok(client)
    }

    async fn negotiate_version(&mut self) -> Result<(), Error> {
        debug!("doing version negotation");
        let version = message::Version::new(&[1]);
        self.control_stream.send_message(version).await?;
        let response = message::VersionResponse::recv(self.control_stream.recv()).await?;
        trace!("negotation response from server {:?}", response);
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
