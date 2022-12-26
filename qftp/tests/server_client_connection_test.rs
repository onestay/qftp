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

#[cfg(test)]
mod test {
    use qftp::{Client, Server};
    use tracing::Level;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn successful_server_client_connection() {
        tracing_subscriber::fmt()
            .with_max_level(Level::TRACE)
            .init();

        let (cert, priv_key) = super::read_test_certs();
        let server = tokio::spawn(async {
            let server =
                Server::new("127.0.0.1:2345".parse().unwrap(), cert, priv_key)
                    .await
                    .unwrap();
            let mut connected_client = server.accept().await.unwrap();
            connected_client.shutdown().await.unwrap();
        });

        let client = tokio::spawn(async {
            std::env::set_var("SSLKEYLOGFILE", "client.keylog");
            let _ = Client::new("127.0.0.1:2345".parse().unwrap())
                .await
                .unwrap();
        });

        futures::future::join_all(vec![server, client]).await;
    }
}
