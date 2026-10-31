use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use wprs_format::Fetch;
use wprs_providers::HttpFetch;

struct Tripwire(Arc<AtomicBool>, u64);

impl Write for Tripwire {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.store(true, Ordering::Relaxed);
        self.1 += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn cancelling_a_stalled_download_returns_at_once() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap());
    thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let _ = stream.read(&mut [0; 1024]);
            stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100000\r\n\r\nx").unwrap();
            thread::sleep(Duration::from_secs(60));
        }
    });
    let cancel = Arc::new(AtomicBool::new(false));
    let fetch = HttpFetch::new(cancel.clone()).unwrap();
    let flag = cancel.clone();
    let start = Instant::now();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        flag.store(true, Ordering::Relaxed);
    });
    assert_eq!(fetch.get(&base, "stalled", 1 << 20).unwrap_err().kind(), io::ErrorKind::Interrupted);
    assert!(start.elapsed() < Duration::from_secs(5), "{:?}", start.elapsed());
}

#[test]
fn cancelling_a_local_copy_stops_it() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("big"), vec![7; 4 << 20]).unwrap();
    let base = format!("file://{}/", dir.path().display());
    let cancel = Arc::new(AtomicBool::new(false));
    let fetch = HttpFetch::new(cancel.clone()).unwrap();
    let mut out = Tripwire(cancel.clone(), 0);
    assert_eq!(fetch.copy(&base, "big", 8 << 20, &mut out).unwrap_err().kind(), io::ErrorKind::Interrupted);
    assert!(out.1 > 0 && out.1 < 4 << 20, "{}", out.1);
    assert_eq!(fetch.get(&base, "big", 8 << 20).unwrap_err().kind(), io::ErrorKind::Interrupted);
    cancel.store(false, Ordering::Relaxed);
    assert_eq!(fetch.get(&base, "big", 10).unwrap().map(|b| b.len()), Some(11));
}
