use std::sync::Arc;

use color_eyre::eyre::Result;
use quinn::Endpoint;
use rustls::client::{ServerCertVerified, ServerCertVerifier};
use rustls::{Certificate, ClientConfig, RootCertStore};
struct DontVerify;

impl ServerCertVerifier for DontVerify {
    fn verify_server_cert(
        &self,
        _: &Certificate,
        _: &[Certificate],
        _: &rustls::ServerName,
        _: &mut dyn Iterator<Item = &[u8]>,
        _: &[u8],
        _: std::time::SystemTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    let mut client = Endpoint::client("0.0.0.0:0".parse()?)?;
    let mut root_store = RootCertStore::empty();
    //let cert = fs::read("cert/ca.crt.de").wrap_err("Failed to read the CA Certificate from disk")?;
    //root_store.add(&Certificate(cert))?;
    for cert in rustls_native_certs::load_native_certs()? {
        root_store.add(&Certificate(cert.0))?;
    }

    let mut client_config = ClientConfig::builder()
        .with_safe_defaults()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    client_config
        .dangerous()
        .set_certificate_verifier(Arc::new(DontVerify {}));
    client.set_default_client_config(quinn::ClientConfig::new(Arc::new(
        client_config,
    )));
    let connecting = client
        .connect("127.0.0.1:3578".parse()?, "miu.local")?
        .await?;
    let (mut send, _) = connecting.open_bi().await?;
    send.write_all("Hello World".as_bytes()).await?;
    send.finish().await?;
    Ok(())
}
