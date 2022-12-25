use thiserror::Error;
mod server;
mod client;
pub mod connected_client;
pub mod message;
mod control_stream;
pub use control_stream::ControlStream;
pub use server::Server;
pub use client::Client;

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
    #[error("Version negotiation failed")]
    NegotiationError,
    #[error("Unknown MessageID `{0}`")]
    MessageIDError(u8),
}