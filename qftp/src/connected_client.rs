use crate::auth::{AuthManager, FileStorage, User};
use crate::control_stream::ControlStream;
use crate::files::{FileManager, QFile};
use crate::message;
use crate::{message::Message, Error};
use quinn::{Connection, SendStream};
use std::sync::Arc;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, error, trace, warn};
const SERVER_SUPPORTED_VERSION: [u8; 1] = [1];

#[derive(Debug)]
pub struct ConnectedClient {
    connection: Connection,
    control_stream: ControlStream,
    user: Option<User>,
    file_manager: Arc<FileManager>,
    running_requests: Vec<RunningRequest>,
}

#[derive(Debug)]
struct RunningRequest {
    handle: JoinHandle<()>,
    cancel_ctx: oneshot::Sender<()>,
}

#[derive(Debug)]
struct RequestContext {
    connection: Connection,
    file_manager: Arc<FileManager>,
    #[allow(dead_code)]
    cancel_ctx: oneshot::Receiver<()>,
}

impl RequestContext {
    fn new(connected_client: &ConnectedClient) -> (Self, oneshot::Sender<()>) {
        let (send, recv) = oneshot::channel();

        let ctx = RequestContext {
            connection: connected_client.connection.clone(),
            file_manager: connected_client.file_manager.clone(),
            cancel_ctx: recv,
        };

        (ctx, send)
    }
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
        let control_stream = ControlStream::new(control_stream.0, control_stream.1);
        let mut connected_client = ConnectedClient {
            connection,
            control_stream,
            user: None,
            file_manager,
            running_requests: Vec::new(),
        };

        connected_client.negotiate_version().await?;
        connected_client.user = Some(connected_client.login(auth_manager).await?);
        Ok(connected_client)
    }

    pub async fn shutdown(mut self) -> Result<(), Error> {
        debug!("shutting down the server");
        trace!("calling finish on the SendStream of the ControlStream");
        match self.control_stream.send().finish().await {
            Ok(()) => (),
            Err(quinn::WriteError::ConnectionLost(quinn::ConnectionError::ApplicationClosed(
                e,
            ))) => {
                if e.error_code != quinn::VarInt::from_u32(0) {
                    warn!("WriteError on ControlStream finish non zero error code: {e:?}");
                }
            }

            Err(e) => {
                warn!("WriteError on ControlStream finish: {e:?}")
            }
        };
        trace!("calling finish on the SendStream of the ControlStream returned");

        trace!("checking all requests");
        for request in self.running_requests {
            if !request.handle.is_finished() {
                request.cancel_ctx.send(()).unwrap();
            }
        }
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
                let (ctx, send) = RequestContext::new(self);

                let handle = tokio::spawn(async move {
                    match ConnectedClient::handle_list_files_request(ctx, request).await {
                        Ok(()) => {
                            debug!("ListFileRequest successfully handled")
                        }
                        Err(e) => {
                            error!("ListFileRequest failed: {e}")
                        }
                    }
                });

                self.running_requests.push(RunningRequest {
                    handle,
                    cancel_ctx: send,
                });
            }
            message::Request::GetFilesRequest(request) => {
                let (ctx, send) = RequestContext::new(self);

                let handle = tokio::spawn(async move {
                    match ConnectedClient::handle_get_files_request(ctx, request).await {
                        Ok(()) => {
                            debug!("GetFilesRequest successfully handled")
                        }
                        Err(e) => {
                            error!("GetFilesRequest failed: {e}")
                        }
                    }
                });

                self.running_requests.push(RunningRequest {
                    handle,
                    cancel_ctx: send,
                });
            }
        }

        Ok(())
    }

    async fn handle_list_files_request(
        ctx: RequestContext,
        request: message::ListFilesRequest,
    ) -> Result<(), Error> {
        trace!("got request {request:#?}\nopening new uni stream");
        let mut uni = ctx.connection.open_uni().await?;

        trace!("opened new uni stream. Sending request_id");
        uni.write_u32(request.request_id()).await?;
        trace!("wrote the request ID");

        let files: Vec<message::ListFileResponse> = ctx
            .file_manager
            .walk_dir("")
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.into())
            .collect();
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

    async fn handle_get_files_request(
        ctx: RequestContext,
        request: message::GetFilesRequest,
    ) -> Result<(), Error> {
        // the purpose of this function is to basically just open the streams and write the reqeust ID
        // the actual logic is implemented in handle_get_files_request_impl
        let mut streams = Vec::new();
        let mut join_set = tokio::task::JoinSet::new();
        trace!("created the join set, spawning streams");
        // TODO: have some upper limit for amount of channels to be spawned to stop a DOS attack
        for i in 0..request.num_streams() {
            trace!("spawning stream {i}");
            let connection = ctx.connection.clone();
            let request_id = request.request_id();

            join_set.spawn(async move {
                let stream = connection.open_uni().await;
                let stream: Result<SendStream, Error> = match stream {
                    Ok(mut stream) => {
                        trace!("stream {i} has been connected. sending request_id {request_id}");
                        match stream.write_u32(request_id).await {
                            Ok(()) => {
                                trace!("stream {i} request_id {request_id} has been written");
                                Ok(stream)
                            }
                            Err(e) => Err(e.into()),
                        }
                    }
                    Err(e) => Err(e.into()),
                };

                stream
            });
        }

        trace!("joining all stream creation threads");
        while let Some(stream) = join_set.join_next().await {
            streams.push(stream.expect("JoinError")?);
        }

        trace!("all streams collected, calling handle_get_files_request_impl");

        ConnectedClient::handle_get_files_request_impl(ctx.file_manager.clone(), streams, request)
            .await
    }

    async fn handle_get_files_request_impl<T>(
        file_manager: Arc<FileManager>,
        mut streams: Vec<T>,
        request: message::GetFilesRequest,
    ) -> Result<(), Error>
    where
        T: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let files = file_manager.walk_dir("").await.unwrap();
        let mut join_set: tokio::task::JoinSet<Result<(), Error>> = tokio::task::JoinSet::new();
        let mut channels = Vec::new();
        for i in 0..request.num_streams() {
            // TODO: it's probably better to use a not unbounded channel here(?)
            let (send, mut recv) = mpsc::unbounded_channel::<QFile>();
            channels.push(send);
            trace!("spawning thread {i} to handle file sending");
            let mut writer = streams
                .pop()
                .expect("we have less streams than requested in num_streams");

            join_set.spawn(async move {
                while let Some(mut file) = recv.recv().await {
                    trace!("Got {file:?} to send");
                    file.send(&mut writer).await?;
                }
                trace!("Got None");
                Ok(())
            });
        }

        // TODO: this is pretty inefficient. A better way would probably be to use select or something
        // and make the threads signal when they are ready for another message
        for (i, file) in files.into_iter().enumerate() {
            channels[i % channels.len()]
                .send(file)
                .expect("couldn't send");
        }

        for channel in channels {
            drop(channel);
        }

        while let Some(res) = join_set.join_next().await {
            match res {
                Ok(Ok(())) => (),
                Ok(Err(e)) => error!("Error in handle_get_files_request_impl worker thread: {e}"),
                Err(e) => error!(
                    "JoinError while joining handle_get_files_request_impl worker threads: {e}"
                ),
            }
        }

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
        debug!("finished version negotiation");
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use tracing::Level;
    use tracing_subscriber::EnvFilter;

    use super::*;

    #[tokio::test]
    async fn test_handle_get_files_request_impl() {
        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("qftp=trace"));
        tracing_subscriber::fmt()
            .with_max_level(Level::TRACE)
            .with_env_filter(env_filter)
            .init();
        let request = message::GetFilesRequest::new(String::from("a"), 1);
        let path = format!("{}/tests/walk_dir", env!("CARGO_MANIFEST_DIR"));
        let file_manager =
            Arc::new(FileManager::new(path).expect("expect creating a file manager not to fail"));
        let a = vec![vec![]];
        ConnectedClient::handle_get_files_request_impl(file_manager, a, request)
            .await
            .expect("expect this not to panic");
    }
}
