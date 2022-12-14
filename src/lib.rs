use thiserror::Error;
pub mod server;
pub mod client;
pub mod connected_client;
pub mod messages;

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
    NegotiationError
}