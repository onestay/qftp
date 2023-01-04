use rustls::{Certificate, PrivateKey};
use std::fs;

fn read_test_certs() -> (Certificate, PrivateKey) {
    let cert = fs::read("cert/dev.crt.der").expect("Failed to read certificate");
    let cert = Certificate(cert);
    let priv_key =
        fs::read("cert/dev.key.der").expect("Failed to read private key");
    let priv_key = PrivateKey(priv_key);

    (cert, priv_key)
}

#[cfg(test)]
mod test {
    use qftp::{Client, ClientBuilder, QClientConfig, Server};
    use tracing::Level;
    use tracing_subscriber::filter::EnvFilter;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn successful_server_client_connection() {
        std::env::set_var("RUST_BACKTRACE", "full");
        // let env_filter = EnvFilter::try_from_default_env()
        //     .unwrap_or_else(|_| EnvFilter::new("qftp=trace"));
        // tracing_subscriber::fmt()
        //     .with_max_level(Level::TRACE)
        //     .with_env_filter(env_filter)
        //     .init();

        let (cert, priv_key) = super::read_test_certs();
        let server = tokio::spawn(async {
            let server =
                Server::new("127.0.0.1:2345".parse().unwrap(), cert, priv_key)
                    .await
                    .unwrap();
            let mut connected_client = server.accept().await.unwrap();
            connected_client
                .next_request()
                .await
                .expect("next request returnd err");
            connected_client.shutdown().await.unwrap();
        });

        let client = tokio::spawn(async {
            std::env::set_var("SSLKEYLOGFILE", "client.keylog");
            let client_config = QClientConfig::dangerous_dont_verify();
            let mut client = Client::builder()
                .set_addr("127.0.0.1:2345", "dev.local".to_string())
                .with_client_config(client_config.into())
                .build()
                .await
                .expect("error constructing the client");
            let result = client.list_files().await.unwrap();
            assert_eq!(result.len(), 4);
            //println!("{result:#?}")
        });

        futures::future::join_all(vec![server, client]).await;
    }
}
