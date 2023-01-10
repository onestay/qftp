#[cfg(test)]
mod test {
    use qftp::{Client, QClientConfig, Server};
    use rustls::{Certificate, PrivateKey};
    use std::{fs, path::PathBuf, str::FromStr};
    use tracing::Level;
    use tracing_subscriber::filter::EnvFilter;
    fn read_test_certs() -> (Certificate, PrivateKey) {
        let cert =
            fs::read("cert/dev.crt.der").expect("Failed to read certificate");
        let cert = Certificate(cert);
        let priv_key =
            fs::read("cert/dev.key.der").expect("Failed to read private key");
        let priv_key = PrivateKey(priv_key);

        (cert, priv_key)
    }

    async fn new_default_server() -> Server {
        let (cert, priv_key) = read_test_certs();
        let path = format!("{}/tests/walk_dir", env!("CARGO_MANIFEST_DIR"));
        let auth_file = format!("{}/tests/auth.json", env!("CARGO_MANIFEST_DIR"));
        let server = Server::builder()
            .set_listen_addr("0.0.0.0:2345".parse().unwrap())
            .set_base_path(PathBuf::from_str(&path).unwrap())
            .set_auth_file(PathBuf::from_str(&auth_file).unwrap())
            .with_certs(vec![cert], priv_key)
            .build()
            .await
            .unwrap();

        server
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn successful_server_client_connection() {
        std::env::set_var("RUST_BACKTRACE", "full");
        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("qftp=trace"));
        tracing_subscriber::fmt()
            .with_max_level(Level::TRACE)
            .with_env_filter(env_filter)
            .init();

        let server = tokio::spawn(async {
            let server = new_default_server().await;
            let mut connected_client = server.accept().await.unwrap();
            connected_client
                .next_request()
                .await
                .expect("next request returned err");
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
            client.shutdown().await.unwrap();
            assert_eq!(result.len(), 4);
            println!("{result:#?}")
        });

        futures::future::join_all(vec![server, client]).await;
    }
}
