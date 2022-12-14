use quinn::{Connection, SendStream, RecvStream};
use tokio::io::AsyncReadExt;
use crate::messages::{MessageID, VersionMessage, Sendable};
use crate::Error;

use tracing::{debug, error, info, span, warn, Level};

const SERVER_SUPPORTED_VERSION: [u8; 1] = [1];


pub struct ConnectedClient {
    connection: Connection,
    control_stream: (SendStream, RecvStream)
}

impl ConnectedClient {
    pub async fn new(connection: Connection) -> Result<Self, Error> {
        debug!("Creating new ConnectedClient");
        let control_stream = connection.accept_bi().await?;
        debug!("Accepted control_stream");
        let mut connected_client = ConnectedClient { connection, control_stream };
        connected_client.negotiate_version().await?;
        Ok(connected_client)
    }

    async fn negotiate_version(&mut self) -> Result<(), Error> {
        // first bytes: message ID
        let message_id = self.control_stream.1.read_u8().await?;
        if message_id != MessageID::HELLO_MESSAGE {
            panic!("Expected HELLO_MESSAGE but got {}", message_id);
        }
        debug!("Read message_id: {}", message_id);
        // second byte: length of versions array
        let len = self.control_stream.1.read_u8().await?;
        debug!("Read legnth: {}", len);
        // read len number of supported versions
        for _ in 0..len {
            if self.control_stream.1.read_u8().await? == SERVER_SUPPORTED_VERSION[0] {
                debug!("Negotatied version 1");
                self.control_stream.0.write_all(&VersionMessage::new(SERVER_SUPPORTED_VERSION[0]).to_bytes()).await?;
                return Ok(());
            }
        }


        Err(Error::NegotiationError)
    }
}

impl ConnectedClient {
    pub async fn get_file(&self) {}
    pub async fn list_files(&self) {}
    pub async fn get_files(&self) {}
}