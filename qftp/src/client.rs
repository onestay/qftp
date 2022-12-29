use crate::{Error, message::{self, Message}, distributor::{StreamRequest, self}};
use quinn::{Connection, Endpoint, SendStream, RecvStream};
use rustls::{
    client::{ServerCertVerified, ServerCertVerifier},
    Certificate, ClientConfig,
    KeyLogFile
};

use tokio::{sync::{oneshot::{self, Sender, Receiver}, mpsc::{self, UnboundedSender, UnboundedReceiver}}, io::AsyncReadExt};

use tracing::{debug, trace};
use crate::ControlStream;
use std::{net::SocketAddr, sync::Arc, collections::HashMap};

/// Entrypoint for creating a qftp Client
#[derive(Debug)]
pub struct Client {
    control_stream: ControlStream,
    recv_stream_request: UnboundedSender<StreamRequest>
}

impl Client {
    fn create_endpoint() -> Result<Endpoint, Error> {
        debug!("Creating client config");
        let mut client_config = ClientConfig::builder()
            .with_safe_defaults()
            .with_custom_certificate_verifier(Arc::new(DontVerify {}))
            .with_no_client_auth();
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
    pub async fn new(addr: SocketAddr) -> Result<Self, Error> {
        let client = Client::create_endpoint()?;
        debug!("Connecting to server");
        let connection = client.connect(addr, "test.server")?.await?;

        debug!("Opening control_stream");
        let control_stream = connection.open_bi().await?;
        let mut control_stream = ControlStream::new(control_stream.0, control_stream.1);

        Client::negotiate_version(&mut control_stream).await?;
        Client::login(&mut control_stream).await?;
        
        let (tx, rx) = mpsc::unbounded_channel();

        let client = Client {
            control_stream,
            recv_stream_request: tx
        };

        tokio::spawn(distributor::run(connection, rx));

        Ok(client)
    }


    async fn negotiate_version(control_stream: &mut ControlStream) -> Result<u8, Error> {
        debug!("doing version negotation");
        let version = message::Version::new(&[1]);
        control_stream.send_message(version).await?;
        let response = message::VersionResponse::recv(control_stream.recv()).await?;
        trace!("negotation response from server {:?}", response);
        Ok(response.negotiated_version)
    }

    async fn login<'a>(control_stream: &mut ControlStream) -> Result<(), Error> {
        let login_request_message = message::LoginRequest::new("test_user".to_string(), "123".to_string());
        control_stream.send_message(login_request_message).await?;
        match control_stream.recv_message::<message::LoginResponse>().await?.is_ok() {
            true => Ok(()),
            false => Err(Error::LoginError)
        }
    }


}

impl Client {
    pub async fn list_files(&mut self) -> Result<Vec<message::ListFileResponse>, Error>{
        let list_files_request = message::ListFilesRequest::new("/".to_string());

        let request_id = list_files_request.request_id();
        self.control_stream.send_message(list_files_request).await?;
        let (tx, rx) = oneshot::channel();
        let req = StreamRequest::new(1, request_id, tx);
        trace!("sending recv_stream_request");
        self.recv_stream_request.send(req).map_err(|_| Error::RequestDistributorChannelSendError)?;
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
        println!("{response:#?}");

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
