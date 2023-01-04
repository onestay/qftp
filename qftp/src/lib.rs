#![deny(rustdoc::broken_intra_doc_links)]
#![deny(missing_debug_implementations)]

use thiserror::Error;
pub mod auth;
mod client;
pub use client::{Client, ClientBuilder, QClientConfig};
pub mod connected_client;
mod control_stream;
mod distributor;
pub mod files;
pub mod message;
mod server;
pub use control_stream::ControlStream;
pub use server::Server;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Crypto error")]
    CryptoError(#[from] rustls::Error),
    #[error("IO error")]
    IOError(#[from] std::io::Error),
    #[error("Connection error")]
    ConnectionError(#[from] quinn::ConnectionError),
    #[error("Connect error")]
    ConnectError(#[from] quinn::ConnectError),
    #[error("QUIC write error")]
    WriteError(#[from] quinn::WriteError),
    #[error("serde_json error")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("Authentication error")]
    AuthenticationError(#[from] crate::auth::AuthError),
    #[error("Version negotiation failed")]
    NegotiationError,
    #[error("Unknown MessageID `{0}`")]
    MessageIDError(u16),
    #[error("The server didn't accept the credentials")]
    LoginError,
    #[error("file error")]
    FileError(#[from] crate::files::FileError),
    #[error("error sending message from request to channel distributor")]
    RequestDistributorChannelSendError,
    #[error("error")]
    RecvErrorOneshot(#[from] tokio::sync::oneshot::error::RecvError),
}
