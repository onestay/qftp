use crate::auth::{AuthManager, FileStorage, User};
use crate::control_stream::ControlStream;
use crate::files::FileManager;
use crate::message;
use crate::{message::Message, Error};
use quinn::Connection;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::{debug, trace};
const SERVER_SUPPORTED_VERSION: [u8; 1] = [1];

#[derive(Debug)]
pub struct ConnectedClient {
    connection: Connection,
    control_stream: ControlStream,
    user: Option<User>,
    file_manager: Arc<FileManager>,
}

impl ConnectedClient {
    pub(crate) async fn new(
        connection: Connection,
        auth_manager: Arc<Mutex<AuthManager<FileStorage>>>,
        file_manager: Arc<FileManager>,
    ) -> Result<Self, Error> {
        trace!("creating new ConnectedClient");
        let control_stream = connection.accept_bi().await?;
        trace!("accepted the control_stream");
        let control_stream =
            ControlStream::new(control_stream.0, control_stream.1);
        let mut connected_client = ConnectedClient {
            connection,
            control_stream,
            user: None,
            file_manager,
        };

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
    async fn login(
        &mut self,
        auth_manager: Arc<Mutex<AuthManager<FileStorage>>>,
    ) -> Result<User, Error> {
        let login_request_message: message::LoginRequest =
            self.control_stream.recv_message().await?;
        let mut auth_manager = auth_manager.lock().await;

        match (*auth_manager)
            .get_user(
                login_request_message.name(),
                login_request_message.password(),
            )
            .await
        {
            Ok(user) => {
                self.control_stream
                    .send_message(message::LoginResponse::new(true))
                    .await?;
                Ok(user)
            }
            Err(e) => {
                self.control_stream
                    .send_message(message::LoginResponse::new(false))
                    .await?;
                Err(e)
            }
        }
    }
    pub async fn next_request(&mut self) -> Result<(), Error> {
        match message::Request::next_request(self.control_stream.recv()).await? {
            message::Request::ListFileRequest(request) => {
                self.handle_list_files_request(request).await?;
            }
        }

        Ok(())
    }

    async fn handle_list_files_request(
        &mut self,
        request: message::ListFilesRequest,
    ) -> Result<(), Error> {
        trace!("got request {request:#?}\nopening new uni stream");
        let mut uni = self.connection.open_uni().await?;
        trace!("opened new uni stream. Sending request_id");
        uni.write_u32(request.request_id()).await?;
        trace!("wrote the request ID");
        let files = self.file_manager.walk_dir("").await.unwrap();
        let msg = message::ListFileResponseHeader {
            num_files: files.len() as u32,
        };
        trace!("sending ListFileResponseHeader {msg:?}");
        msg.send(&mut uni).await?;
        trace!("sending files");
        for file in files {
            file.send(&mut uni).await?;
        }
        trace!("done sending files");
        uni.finish().await?;
        Ok(())
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
