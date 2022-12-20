use rustls::{Certificate, PrivateKey};
use std::fs;

fn read_test_certs() -> (Certificate, PrivateKey) {
    let cert =
        fs::read("cert/miu.local.crt").expect("Failed to read certificate");
    let cert = Certificate(cert);
    let priv_key =
        fs::read("cert/miu.local.der").expect("Failed to read private key");
    let priv_key = PrivateKey(priv_key);

    (cert, priv_key)
}

mod test {
    use qftp::{server::Server, client::Client};
    use tracing::{info, Level};
    use tracing_subscriber::FmtSubscriber;
    #[tokio::test]
    async fn successful_server_client_connection() {
        let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();

        tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

        let (cert, priv_key) = super::read_test_certs();
        let server = tokio::spawn(async {
            let server = Server::new("127.0.0.1:2345".parse().unwrap(), cert, priv_key).unwrap();
            let _ = server.accept().await.unwrap();
        });

        let client = tokio::spawn(async {
            let _ = Client::new("127.0.0.1:2345".parse().unwrap()).await.unwrap();
        });

        futures::future::join_all(vec![server, client]).await;
    }
}