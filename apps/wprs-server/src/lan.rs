use anyhow::{Context, Result};
use axum::serve::Listener;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::net::SocketAddr;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_rustls::rustls::ServerConfig;
use tokio_rustls::rustls::crypto::aws_lc_rs;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio_rustls::server::TlsStream;

pub const SERVICE: &str = "_wprs._tcp.local.";

pub fn wanted() -> Result<bool> {
    match std::env::var("WPRS_TLS").unwrap_or_default().trim() {
        "" | "off" => Ok(false),
        "self-signed" => Ok(true),
        other => anyhow::bail!("WPRS_TLS must be off or self-signed, not {other}"),
    }
}

pub fn advertised() -> Result<bool> {
    match std::env::var("WPRS_ADVERTISE").unwrap_or_default().trim() {
        "" | "false" | "0" | "off" => Ok(false),
        "true" | "1" | "on" => Ok(true),
        other => anyhow::bail!("WPRS_ADVERTISE must be true or false, not {other}"),
    }
}

pub fn host(public_url: &str) -> String {
    let rest = public_url.split_once("://").map_or(public_url, |(_, rest)| rest);
    let host = rest.split('/').next().unwrap_or_default();
    match host.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or_default().to_owned(),
        None => host.rsplit_once(':').map_or(host, |(name, _)| name).to_owned(),
    }
}

fn keep(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new().create_new(true).write(true).mode(0o600).open(path)?;
    Ok(file.write_all(bytes)?)
}

pub fn certificate(data: &Path, host: &str) -> Result<(Vec<u8>, Vec<u8>)> {
    let dir = data.join("tls");
    let (cert, key) = (dir.join("cert.der"), dir.join("key.der"));
    if let (Ok(c), Ok(k)) = (fs::read(&cert), fs::read(&key)) {
        return Ok((c, k));
    }
    fs::create_dir_all(&dir)?;
    let names = vec![host.to_owned(), "localhost".to_owned()];
    let made = rcgen::generate_simple_self_signed(names).context("cannot make the server certificate")?;
    let (c, k) = (made.cert.der().to_vec(), made.signing_key.serialize_der());
    keep(&key, &k)?;
    keep(&cert, &c)?;
    Ok((c, k))
}

pub fn fingerprint(der: &[u8]) -> String {
    Sha256::digest(der).iter().map(|b| format!("{b:02x}")).collect()
}

pub fn acceptor(cert: Vec<u8>, key: Vec<u8>) -> Result<tokio_rustls::TlsAcceptor> {
    let provider = Arc::new(aws_lc_rs::default_provider());
    let config = ServerConfig::builder_with_provider(provider).with_safe_default_protocol_versions()?;
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key));
    let config = config.with_no_client_auth().with_single_cert(vec![CertificateDer::from(cert)], key)?;
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(config)))
}

pub struct Tls {
    shaken: mpsc::Receiver<(TlsStream<TcpStream>, SocketAddr)>,
    local: SocketAddr,
}

pub fn tls(tcp: TcpListener, acceptor: tokio_rustls::TlsAcceptor) -> Result<Tls> {
    let (sent, shaken) = mpsc::channel(64);
    let local = tcp.local_addr()?;
    tokio::spawn(async move {
        loop {
            let Ok((socket, peer)) = tcp.accept().await else {
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            };
            let (acceptor, sent) = (acceptor.clone(), sent.clone());
            tokio::spawn(async move {
                let shaking = tokio::time::timeout(Duration::from_secs(10), acceptor.accept(socket));
                if let Ok(Ok(stream)) = shaking.await {
                    drop(sent.send((stream, peer)).await);
                }
            });
        }
    });
    Ok(Tls { shaken, local })
}

impl Listener for Tls {
    type Io = TlsStream<TcpStream>;
    type Addr = SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        match self.shaken.recv().await {
            Some(found) => found,
            None => std::future::pending().await,
        }
    }

    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        Ok(self.local)
    }
}

pub fn advertise(port: u16) -> Result<mdns_sd::ServiceDaemon> {
    let daemon = mdns_sd::ServiceDaemon::new().context("cannot start mDNS")?;
    let name = format!("wallpaper-rs-{}", fingerprint(&getrandom::u64()?.to_le_bytes())[..6].to_owned());
    let host = format!("{name}.local.");
    let info =
        mdns_sd::ServiceInfo::new(SERVICE, &name, &host, "", port, &[("v", "0")][..])?.enable_addr_auto();
    daemon.register(info).context("cannot announce the server on the local network")?;
    Ok(daemon)
}
