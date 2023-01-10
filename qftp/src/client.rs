use crate::{
    distributor::{self, StreamRequest},
    message::{self, Message},
    Error,
};
use quinn::Endpoint;
use rustls::{
    client::{ServerCertVerified, ServerCertVerifier},
    Certificate, ClientConfig, KeyLogFile, RootCertStore,
};

use tokio::{
    io::AsyncWriteExt,
    sync::{
        mpsc::{self, UnboundedSender},
        oneshot,
    },
};

use crate::ControlStream;
use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
};
use tracing::{debug, trace};

/// A simple wrapper around [Rustls ClientConfig](rustls::ClientConfig)
#[derive(Debug)]
pub struct QClientConfig {
    client_config: ClientConfig,
}

impl QClientConfig {
    /// Creates a new [QClientConfig] with the Operating Systems root cert store. Check the [rustls_native_certs](rustls_native_certs::load_native_certs) crate for more information.
    ///
    /// Requirers the "native-certs" feature to be enabled.
    #[cfg(feature = "native-certs")]
    pub fn with_native_certs() -> Self {
        use rustls_native_certs::load_native_certs;
        let certs = load_native_certs().expect("failed to load native certs");
        let mut store = RootCertStore::empty();
        for cert in certs {
            store
                .add(&Certificate(cert.0))
                .expect("failed to add certificate to store")
        }

        let config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(store)
            .with_no_client_auth();

        QClientConfig {
            client_config: config,
        }
    }

    /// Creates a new [QClientConfig] with the specified certificates. Certificate needs to be a DER-encoded X.509 certificate as described in [Rustls](rustls::Certificate)
    pub fn with_certs(certs: Vec<Certificate>) -> Self {
        let mut store = RootCertStore::empty();
        for cert in certs {
            store
                .add(&Certificate(cert.0))
                .expect("failed to add certificate to store")
        }

        let config = ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(store)
            .with_no_client_auth();

        QClientConfig {
            client_config: config,
        }
    }

    /// Creates a new [QClientConfig]. This config will not verify the client certificates. This is potentially very dangerous.
    pub fn dangerous_dont_verify() -> Self {
        let config = ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(Arc::new(DontVerify {}))
            .with_no_client_auth();

        QClientConfig {
            client_config: config,
        }
    }
}

impl From<QClientConfig> for ClientConfig {
    fn from(value: QClientConfig) -> Self {
        value.client_config
    }
}

/// Builder for a [Client]. Usually created by [Client::builder]
#[derive(Debug)]
pub struct ClientBuilder {
    addr: Option<SocketAddr>,
    server_name: Option<String>,
    config: Option<ClientConfig>,
}

impl ClientBuilder {
    /// Set the address to connect to.
    /// `T` will be resolved to the first IPv4 address.
    pub fn set_addr<T: ToSocketAddrs>(
        mut self,
        addr: T,
        server_name: String,
    ) -> Self {
        let mut addr = addr
            .to_socket_addrs()
            .expect("{addr} is not a valid SocketAddress");
        let addr = addr
            .find(|a| matches!(a, SocketAddr::V4(..)))
            .expect("{addr} didn't resolve to a valid IPv4 addr");
        self.addr = Some(addr);
        self.server_name = Some(server_name);

        self
    }

    /// Set the [ClientConfig](rustls::ClientConfig). Can be constructed with [QClientConfig]
    pub fn with_client_config(mut self, config: ClientConfig) -> Self {
        self.config = Some(config);

        self
    }

    pub async fn build(self) -> Result<Client, Error> {
        Client::new(
            self.addr
                .expect("tried calling build without setting the addr"),
            self.server_name
                .expect("tried calling build without setting the server name"),
            self.config
                .expect("tried calling build without setting the client_config"),
        )
        .await
    }
}

/// Entrypoint for creating a qftp Client
#[derive(Debug)]
pub struct Client {
    control_stream: ControlStream,
    recv_stream_request: UnboundedSender<StreamRequest>,
}

impl Client {
    pub fn builder() -> ClientBuilder {
        ClientBuilder {
            addr: None,
            server_name: None,
            config: None,
        }
    }

    pub async fn shutdown(&mut self) -> Result<(), Error> {
        debug!("shutting down the client");
        trace!("calling finish on the SendStream of the ControlStream");
        self.control_stream.send().finish().await?;
        trace!("calling finish on the SendStream of the ControlStream returned");
        Ok(())
    }

    fn create_endpoint(
        mut client_config: ClientConfig,
    ) -> Result<Endpoint, Error> {
        debug!("Creating client config");
        client_config.key_log = Arc::new(KeyLogFile::new());
        debug!("Creating client");
        let mut client = Endpoint::client(
            "0.0.0.0:0".parse().expect("Failed to parse address"),
        )?;
        client.set_default_client_config(quinn::ClientConfig::new(Arc::new(
            client_config,
        )));

        Ok(client)
    }

    async fn new(
        addr: SocketAddr,
        server_name: String,
        client_config: ClientConfig,
    ) -> Result<Self, Error> {
        let client = Client::create_endpoint(client_config)?;
        debug!("Connecting to server");
        let connection = client.connect(addr, &server_name)?.await?;

        debug!("Opening control_stream");
        let control_stream = connection.open_bi().await?;
        let mut control_stream =
            ControlStream::new(control_stream.0, control_stream.1);

        Client::negotiate_version(&mut control_stream).await?;
        Client::login(&mut control_stream).await?;

        let (tx, rx) = mpsc::unbounded_channel();

        let client = Client {
            control_stream,
            recv_stream_request: tx,
        };

        tokio::spawn(distributor::run(connection, rx));

        Ok(client)
    }

    async fn negotiate_version(
        control_stream: &mut ControlStream,
    ) -> Result<u8, Error> {
        debug!("doing version negotation");
        let version = message::Version::new(&[1]);
        control_stream.send_message(version).await?;
        let response =
            message::VersionResponse::recv(control_stream.recv()).await?;
        trace!("negotation response from server {:?}", response);
        Ok(response.negotiated_version)
    }

    async fn login<'a>(control_stream: &mut ControlStream) -> Result<(), Error> {
        let login_request_message = message::LoginRequest::new(
            "test_user".to_string(),
            "123".to_string(),
        );
        control_stream.send_message(login_request_message).await?;
        match control_stream
            .recv_message::<message::LoginResponse>()
            .await?
            .is_ok()
        {
            true => Ok(()),
            false => Err(Error::LoginError),
        }
    }
}

impl Client {
    pub async fn list_files(
        &mut self,
    ) -> Result<Vec<message::ListFileResponse>, Error> {
        let list_files_request = message::ListFilesRequest::new("/".to_string());

        let request_id = list_files_request.request_id();
        trace!("sending request number");
        self.control_stream.send().write_u16(0x01).await?;
        self.control_stream.send_message(list_files_request).await?;
        let (tx, rx) = oneshot::channel();
        let req = StreamRequest::new(1, request_id, tx);
        trace!("sending recv_stream_request");
        self.recv_stream_request
            .send(req)
            .map_err(|_| Error::RequestDistributorChannelSendError)?;
        trace!("got the streams!");
        let mut streams = rx.await?;
        assert!(streams.len() == 1);

        let uni = &mut streams[0];
        let header = message::ListFileResponseHeader::recv(uni).await?;

        let mut response = Vec::with_capacity(header.num_files as usize);
        for _ in 0..header.num_files {
            let file = message::ListFileResponse::recv(uni).await?;
            response.push(file);
        }

        Ok(response)
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
