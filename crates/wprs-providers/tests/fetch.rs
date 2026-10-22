use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wprs_format::Fetch;
use wprs_providers::HttpFetch;

const AGENT: &str = concat!("wallpaper-rs/", env!("CARGO_PKG_VERSION"));

struct Tripwire(Arc<AtomicBool>, Vec<u8>);

impl Write for Tripwire {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.store(true, Ordering::Relaxed);
        self.1.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn moved(to: String) -> ResponseTemplate {
    ResponseTemplate::new(302).insert_header("location", to)
}

#[tokio::test]
async fn requests_are_generic_capped_and_stay_on_safe_schemes() {
    let server = MockServer::start().await;
    let body = ResponseTemplate::new(200).set_body_bytes(b"0123456789".to_vec());
    let body = body.insert_header("set-cookie", "seen=1");
    let data = Mock::given(method("GET")).and(path("/repo/data")).and(header("user-agent", AGENT));
    data.respond_with(body).mount(&server).await;
    Mock::given(path("/repo/moved"))
        .respond_with(moved(format!("{}/repo/data", server.uri())))
        .mount(&server)
        .await;
    Mock::given(path("/repo/away"))
        .respond_with(moved("http://example.org/repo/data".into()))
        .mount(&server)
        .await;
    let base = format!("{}/repo/", server.uri());
    let (full, capped, missing, away, plain) = tokio::task::spawn_blocking(move || {
        let fetch = HttpFetch::new(Arc::default()).unwrap();
        (
            fetch.get(&base, "data", 100).unwrap(),
            fetch.get(&base, "moved", 4).unwrap(),
            fetch.get(&base, "missing", 100).unwrap(),
            fetch.get(&base, "away", 100).is_err(),
            fetch.get("http://example.org/repo/", "data", 100).unwrap_err().kind(),
        )
    })
    .await
    .unwrap();
    assert_eq!((full.unwrap(), capped.unwrap(), missing), (b"0123456789".to_vec(), b"01234".to_vec(), None));
    assert!(away);
    assert_eq!(plain, io::ErrorKind::PermissionDenied);
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 5);
    for name in ["cookie", "referer", "accept-language"] {
        assert!(requests.iter().all(|r| !r.headers.contains_key(name)), "{name}");
    }
}

#[tokio::test]
async fn cancellation_interrupts_between_chunks() {
    let server = MockServer::start().await;
    let big = ResponseTemplate::new(200).set_body_bytes(vec![7; 1 << 20]);
    Mock::given(method("GET")).respond_with(big).mount(&server).await;
    let base = format!("{}/", server.uri());
    let (during, before, got) = tokio::task::spawn_blocking(move || {
        let cancel = Arc::new(AtomicBool::new(false));
        let fetch = HttpFetch::new(cancel.clone()).unwrap();
        let mut out = Tripwire(cancel, Vec::new());
        let during = fetch.copy(&base, "big", 2 << 20, &mut out).unwrap_err().kind();
        (during, fetch.get(&base, "big", 2 << 20).unwrap_err().kind(), out.1.len())
    })
    .await
    .unwrap();
    assert_eq!((during, before), (io::ErrorKind::Interrupted, io::ErrorKind::Interrupted));
    assert!(got > 0 && got < 1 << 20, "{got}");
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[test]
fn file_urls_and_folders_are_read_locally() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("entry"), b"local").unwrap();
    let fetch = HttpFetch::new(Arc::default()).unwrap();
    for base in [format!("file://{}/", dir.path().display()), format!("{}/", dir.path().display())] {
        assert_eq!(fetch.get(&base, "entry", 100).unwrap(), Some(b"local".to_vec()));
        assert_eq!(fetch.get(&base, "absent", 100).unwrap(), None);
    }
}

#[test]
fn slow_and_stalled_transfers_are_aborted() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    thread::spawn(move || {
        for (mut stream, pause) in listener.incoming().flatten().zip([50, 5000]) {
            let _ = stream.read(&mut [0; 1024]);
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100000\r\n\r\n").unwrap();
            while stream.write_all(b"x").is_ok() {
                thread::sleep(Duration::from_millis(pause));
            }
        }
    });
    let fetch = HttpFetch::with_window(Arc::default(), Duration::from_millis(500)).unwrap();
    let start = Instant::now();
    assert_eq!(fetch.get(&base, "slow", 1 << 20).unwrap_err().kind(), io::ErrorKind::TimedOut);
    assert_eq!(fetch.get(&base, "stalled", 1 << 20).unwrap_err().kind(), io::ErrorKind::TimedOut);
    assert!(start.elapsed() < Duration::from_secs(5));
}
