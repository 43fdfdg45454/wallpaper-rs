use std::path::PathBuf;
use wprs_server::{App, Config, lan};

#[tokio::test(flavor = "multi_thread")]
async fn a_home_server_is_reached_over_https_by_its_pinned_fingerprint() {
    let dir = tempfile::tempdir().unwrap();
    let tcp = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("https://{}", tcp.local_addr().unwrap());
    let vars = [("WPRS_DATA", dir.path().display().to_string()), ("WPRS_PUBLIC_URL", url.clone())];
    let mut cfg =
        Config::from_vars(|key| vars.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone())).unwrap();
    cfg.validator = PathBuf::from(env!("CARGO_BIN_EXE_wprs-server"));
    assert_eq!(lan::host(&url), "127.0.0.1");
    let (cert, key) = lan::certificate(dir.path(), "127.0.0.1").unwrap();
    assert_eq!(lan::certificate(dir.path(), "127.0.0.1").unwrap().0, cert, "the certificate is kept");
    let print = lan::fingerprint(&cert);
    let tls = lan::tls(tcp, lan::acceptor(cert, key).unwrap()).unwrap();
    tokio::spawn(App::open(cfg).unwrap().serve(tls));
    let unknown = wprs_sync::Api::new(&url).unwrap().get("/.well-known/wprs-server").await;
    assert!(matches!(unknown, Err(wprs_sync::Error::Untrusted)), "{unknown:?}");
    assert_eq!(wprs_sync::peek(&url).await.unwrap(), print);
    let found =
        wprs_sync::Api::pinned(&url, Some(&print)).unwrap().get("/.well-known/wprs-server").await.unwrap();
    assert!(found["api"].as_array().unwrap().contains(&serde_json::json!("v1")), "{found}");
    let mode = std::os::unix::fs::PermissionsExt::mode(
        &std::fs::metadata(dir.path().join("tls/key.der")).unwrap().permissions(),
    );
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn hosts_are_taken_from_the_public_address() {
    assert_eq!(lan::host("https://fondos.local:8443/"), "fondos.local");
    assert_eq!(lan::host("https://[fd00::2]:8443"), "fd00::2");
}

#[test]
fn a_certificate_cut_short_by_a_crash_is_made_again() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("tls")).unwrap();
    std::fs::write(dir.path().join("tls/key.der"), b"cut short").unwrap();
    let (cert, key) = lan::certificate(dir.path(), "127.0.0.1").unwrap();
    assert!(lan::acceptor(cert.clone(), key.clone()).is_ok());
    assert_eq!(std::fs::read(dir.path().join("tls/key.der")).unwrap(), key);
    assert_eq!(lan::certificate(dir.path(), "127.0.0.1").unwrap(), (cert, key), "the new pair is kept");
    let names: Vec<_> =
        std::fs::read_dir(dir.path().join("tls")).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(names.len(), 2, "no temporary file is left: {names:?}");
}
