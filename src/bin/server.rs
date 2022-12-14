use color_eyre::eyre::Result;
use quinn::Endpoint;
use rustls::ServerConfig;
use rustls::{Certificate, PrivateKey};
use std::{fs, sync::Arc};
#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    const LEN: usize = "Hello World".len();
    let cert = fs::read("cert/miu.local.crt")?;
    let cert = Certificate(cert);
    let privkey = fs::read("cert/miu.local.der")?;
    let privkey = PrivateKey(privkey);

    let server_config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(vec![cert], privkey)?;
    let server_config = quinn::ServerConfig::with_crypto(Arc::new(server_config));
    let server = Endpoint::server(server_config, "127.0.0.1:3578".parse()?)?;
    let conn = server.accept().await.unwrap().await?;
    let (_, mut recv) = conn.accept_bi().await?;
    let mut buf: [u8; LEN] = [0; LEN];
    recv.read_exact(&mut buf).await?;
    println!("{}", String::from_utf8_lossy(&buf));

    Ok(())
}
