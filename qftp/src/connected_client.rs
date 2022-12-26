use std::sync::Arc;
use tokio::sync::Mutex;
use quinn::Connection;
use crate::{Error, message::Message};
use crate::control_stream::ControlStream;
use tracing::{trace, debug};
use crate::message;
use crate::auth::{User, AuthManager, FileStorage};
const SERVER_SUPPORTED_VERSION: [u8; 1] = [1];


pub struct ConnectedClient {
    connection: Connection,
    control_stream: ControlStream,
    user: Option<User>
}

impl ConnectedClient {
    pub(crate) async fn new(connection: Connection, auth_manager: Arc<Mutex<AuthManager<FileStorage>>>) -> Result<Self, Error> {
        trace!("creating new ConnectedClient");
        let control_stream = connection.accept_bi().await?;
        trace!("accepted the control_stream");
        let control_stream = ControlStream::new(control_stream.0, control_stream.1);
        let mut connected_client = ConnectedClient { connection, control_stream, user: None };

        connected_client.negotiate_version().await?;
        connected_client.user = Some(connected_client.login(auth_manager).await?);

        Ok(connected_client)
    }

    pub async fn shutdown(&mut self) -> Result<(), Error> {
        debug!("shutting down the server");
        trace!("calling finish on the SendStream of the ControlStream");
        self.control_stream.send().finish().await?;
        trace!("calling finish on the SendStream of the ControlStream returned");
        Ok(())
    }
    async fn login(&mut self, auth_manager: Arc<Mutex<AuthManager<FileStorage>>>) -> Result<User, Error> {
        let login_request_message: message::LoginRequest = self.control_stream.recv_message().await?;
        let mut auth_manager = auth_manager.lock().await;
        
        match (*auth_manager).get_user(login_request_message.name(), login_request_message.password()).await {
            Ok(user) => {
                self.control_stream.send_message(message::LoginResponse::new(true)).await?;
                Ok(user)
            },
            Err(e) => {
                self.control_stream.send_message(message::LoginResponse::new(false)).await?;
                Err(e)
            }
        }
        
    }
    async fn negotiate_version(&mut self) -> Result<(), Error> {
        debug!("doing version negotation");
        let version = message::Version::recv(self.control_stream.recv()).await?;
        trace!("negotation message from client {:?}", version);
        for version in version.versions() {
            if SERVER_SUPPORTED_VERSION.contains(version) {
                trace!("version {} negotiated", version);
                let version_response = message::VersionResponse::new(*version);
                self.control_stream.send_message(version_response).await?;
            }
        }
        debug!("finished version negotation");
        Ok(())
    }
}

impl ConnectedClient {
    pub async fn get_file(&self) {}
    pub async fn list_files(&self) {}
    pub async fn get_files(&self) {}
}