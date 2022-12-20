use quinn::{Connection, SendStream, RecvStream};
use tokio::io::AsyncReadExt;
use crate::Error;
use crate::control_stream::ControlStream;
use tracing::{trace, debug, error, info, span, warn, Level};

const SERVER_SUPPORTED_VERSION: [u8; 1] = [1];


pub struct ConnectedClient {
    connection: Connection,
    control_stream: ControlStream
}

impl ConnectedClient {
    pub async fn new(connection: Connection) -> Result<Self, Error> {
        trace!("creating new ConnectedClient");
        let control_stream = connection.accept_bi().await?;
        trace!("accepted the control_stream");
        let control_stream = ControlStream::new(control_stream.0, control_stream.1);
        let mut connected_client = ConnectedClient { connection, control_stream };
        connected_client.negotiate_version().await?;
        Ok(connected_client)
    }

    async fn negotiate_version(&mut self) -> Result<(), Error> {
        // first bytes: message ID
        

        Err(Error::NegotiationError)
    }
}

impl ConnectedClient {
    pub async fn get_file(&self) {}
    pub async fn list_files(&self) {}
    pub async fn get_files(&self) {}
}