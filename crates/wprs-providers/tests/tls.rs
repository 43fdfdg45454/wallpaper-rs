use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
use wiremock::{Mock, MockServer, ResponseTemplate, matchers::any};
use wprs_format::Fetch;
use wprs_providers::{HttpFetch, Provider, WebDav, client};

const PEM: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/loopback-tls.pem");

async fn secure(plain: String) -> String {
    let pem = std::fs::read(PEM).unwrap();
    let chain = CertificateDer::pem_slice_iter(&pem).collect::<Result<Vec<_>, _>>().unwrap();
    let key = PrivateKeyDer::from_pem_slice(&pem).unwrap();
    let config = ServerConfig::builder().with_no_client_auth().with_single_cert(chain, key).unwrap();
    let (acceptor, listener) =
        (TlsAcceptor::from(Arc::new(config)), TcpListener::bind("127.0.0.1:0").await.unwrap());
    let base = format!("https://127.0.0.1:{}/", listener.local_addr().unwrap().port());
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            let (acceptor, plain) = (acceptor.clone(), plain.clone());
            tokio::spawn(async move {
                let Ok(mut tls) = acceptor.accept(socket).await else { return };
                let mut asked = Vec::new();
                while !asked.ends_with(b"\r\n\r\n") && tls.read_u8().await.map(|b| asked.push(b)).is_ok() {}
                let reply = match asked.starts_with(b"GET /ok ") {
                    true => "HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok".to_owned(),
                    false => format!(
                        "HTTP/1.1 302 Found\r\nlocation: {plain}stolen\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
                    ),
                };
                drop(tls.write_all(reply.as_bytes()).await);
                drop(tls.shutdown().await);
            });
        }
    });
    base
}

#[tokio::test(flavor = "multi_thread")]
async fn https_never_redirects_to_plain_http() {
    unsafe { std::env::set_var("SSL_CERT_FILE", PEM) };
    let plain = MockServer::start().await;
    Mock::given(any()).respond_with(ResponseTemplate::new(200).set_body_string("stolen")).mount(&plain).await;
    let base = secure(format!("{}/", plain.uri())).await;
    let web = client().unwrap();
    let ok = web.get(format!("{base}ok")).send().await.unwrap();
    assert_eq!(ok.text().await.unwrap(), "ok");
    assert_eq!(web.get(format!("{base}jump")).send().await.unwrap().status(), 302);
    let dav = WebDav::new(format!("{base}dav/"), "me", "hunter2").unwrap();
    assert!(dav.list().await.is_err());
    let bases = vec![base.clone(), format!("{}/", plain.uri())];
    let (found, jumped) = tokio::task::spawn_blocking(move || {
        let fetch = HttpFetch::new(Arc::default()).unwrap().within(&bases);
        (fetch.get(&base, "ok", 16).unwrap(), fetch.get(&base, "jump", 16))
    })
    .await
    .unwrap();
    assert_eq!(found.as_deref(), Some(&b"ok"[..]));
    assert!(!matches!(jumped, Ok(Some(_))), "{jumped:?}");
    assert!(plain.received_requests().await.unwrap().is_empty());
}
