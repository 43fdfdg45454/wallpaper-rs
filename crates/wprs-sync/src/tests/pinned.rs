use crate::{Api, Error, fingerprint, peek};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

const PEM: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/loopback-tls.pem");

async fn serve() -> (String, String) {
    let pem = std::fs::read(PEM).unwrap();
    let chain = CertificateDer::pem_slice_iter(&pem).collect::<Result<Vec<_>, _>>().unwrap();
    let print = fingerprint(&chain[0]);
    let key = PrivateKeyDer::from_pem_slice(&pem).unwrap();
    let config = ServerConfig::builder().with_no_client_auth().with_single_cert(chain, key).unwrap();
    let (acceptor, listener) =
        (TlsAcceptor::from(Arc::new(config)), TcpListener::bind("127.0.0.1:0").await.unwrap());
    let base = format!("https://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                let Ok(mut tls) = acceptor.accept(socket).await else { return };
                let mut asked = Vec::new();
                while !asked.ends_with(b"\r\n\r\n") && tls.read_u8().await.map(|b| asked.push(b)).is_ok() {}
                let body = r#"{"api": ["v1"]}"#;
                let reply = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                drop(tls.write_all(reply.as_bytes()).await);
                drop(tls.shutdown().await);
            });
        }
    });
    (base, print)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_with_its_own_certificate_is_reached_only_by_its_fingerprint() {
    let (base, print) = serve().await;
    let plain = Api::new(&base).unwrap().get("/.well-known/wprs-server").await;
    assert!(matches!(plain, Err(Error::Untrusted)), "{plain:?}");
    assert_eq!(peek(&base).await.unwrap(), print);
    let pinned = Api::pinned(&base, Some(&print)).unwrap();
    assert_eq!(pinned.get("/.well-known/wprs-server").await.unwrap()["api"][0], "v1");
    let other = Api::pinned(&base, Some(&"0".repeat(64))).unwrap().get("/.well-known/wprs-server").await;
    assert!(matches!(other, Err(Error::Untrusted)), "{other:?}");
    assert!(Api::pinned(&base, Some("not hex")).is_err());
}
