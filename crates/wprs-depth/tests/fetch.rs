use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use wprs_depth::{AGENT, Cancelled, download};

const SIZE: usize = 300_000;

fn body() -> Vec<u8> {
    (0..SIZE).map(|i| (i * 7 % 251) as u8).collect()
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

type Seen = Arc<Mutex<Vec<Vec<String>>>>;

fn serve() -> (String, Seen) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let (base, seen) = (format!("http://{}", listener.local_addr().unwrap()), Seen::default());
    let log = seen.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let reader = BufReader::new(stream.try_clone().unwrap());
            let lines: Vec<String> =
                reader.lines().map_while(Result::ok).take_while(|l| !l.is_empty()).collect();
            let path = lines.first().and_then(|l| l.split(' ').nth(1)).unwrap_or_default().to_owned();
            let from = lines
                .iter()
                .find_map(|l| l.to_ascii_lowercase().strip_prefix("range: bytes=").map(String::from));
            let from: usize = from.and_then(|r| r.trim_end_matches('-').parse().ok()).unwrap_or(0);
            let first = log.lock().unwrap().iter().all(|r: &Vec<String>| !r[0].contains(&path));
            log.lock().unwrap().push(lines);
            let mut data = body();
            data.extend(if path == "/big" { vec![0; 10] } else { vec![] });
            let (status, head) = match (path.as_str(), from) {
                ("/missing", _) => ("404 Not Found", String::new()),
                ("/moved", _) => ("302 Found", "Location: /file\r\n".to_owned()),
                (_, 0) => ("200 OK", String::new()),
                (_, from) => {
                    ("206 Partial Content", format!("Content-Range: bytes {from}-{}/{}\r\n", SIZE - 1, SIZE))
                }
            };
            let rest = if status.starts_with('2') { &data[from..] } else { &[][..] };
            let length =
                if path == "/liar" { String::new() } else { format!("Content-Length: {}\r\n", rest.len()) };
            let mut out = stream;
            let _ = write!(out, "HTTP/1.1 {status}\r\n{head}{length}Connection: close\r\n\r\n");
            let cut = if path == "/cut" && first { rest.len() / 2 } else { rest.len() };
            let _ = out.write_all(&rest[..cut]);
        }
    });
    (base, seen)
}

fn quiet() -> impl FnMut(u64, u64) -> bool {
    |_, _| true
}

fn plain(requests: &[Vec<String>]) {
    for header in requests.iter().flat_map(|r| r.iter().skip(1)) {
        let name = header.split(':').next().unwrap().to_ascii_lowercase();
        assert!(
            ["host", "user-agent", "accept", "range", "accept-encoding"].contains(&name.as_str()),
            "{header}"
        );
    }
}

#[test]
fn interrupted_downloads_resume_with_a_range_and_a_plain_agent() {
    let ((base, seen), dir) = (serve(), tempfile::tempdir().unwrap());
    let dest = dir.path().join("model.onnx");
    let url = format!("{base}/cut");
    let stopped = download(&[&url], &dest, SIZE as u64, &hex(&body()), &mut quiet()).unwrap_err();
    assert!(format!("{stopped:#}").contains("resumes next time"), "{stopped:#}");
    assert_eq!(std::fs::metadata(dest.with_added_extension("part")).unwrap().len(), SIZE as u64 / 2);
    let mut last = 0;
    download(&[&url], &dest, SIZE as u64, &hex(&body()), &mut |done, _| {
        last = done;
        true
    })
    .unwrap();
    assert_eq!((std::fs::read(&dest).unwrap(), last), (body(), SIZE as u64));
    assert!(!dest.with_added_extension("part").exists());
    let requests = seen.lock().unwrap().clone();
    assert!(requests[1].iter().any(|h| h.eq_ignore_ascii_case(&format!("range: bytes={}-", SIZE / 2))));
    plain(&requests);
    assert!(requests[0].iter().any(|h| h.eq_ignore_ascii_case(&format!("user-agent: {AGENT}"))));
}

#[test]
fn redirects_send_only_the_plain_headers() {
    let ((base, seen), dir) = (serve(), tempfile::tempdir().unwrap());
    let dest = dir.path().join("model.onnx");
    download(&[&format!("{base}/moved")], &dest, SIZE as u64, &hex(&body()), &mut quiet()).unwrap();
    let requests = seen.lock().unwrap().clone();
    assert!(requests.len() == 2 && requests[1][0].starts_with("GET /file "), "{requests:?}");
    plain(&requests);
}

#[test]
fn wrong_hashes_and_oversized_files_are_deleted() {
    let ((base, _), dir) = (serve(), tempfile::tempdir().unwrap());
    let dest = dir.path().join("model.onnx");
    let part = dest.with_added_extension("part");
    let wrong =
        download(&[&format!("{base}/file")], &dest, SIZE as u64, &hex(b"other"), &mut quiet()).unwrap_err();
    assert!(wrong.to_string().contains("another SHA-256") && !part.exists() && !dest.exists());
    for path in ["/big", "/liar"] {
        let url = format!("{base}{path}");
        let refused = download(&[&url], &dest, SIZE as u64 - 10, &hex(&body()), &mut quiet()).unwrap_err();
        assert!(format!("{refused:#}").contains("bytes"), "{path}: {refused:#}");
        assert!(!part.exists() && !dest.exists(), "{path}");
    }
    let urls = [format!("{base}/missing"), format!("{base}/file")];
    download(&[&urls[0], &urls[1]], &dest, SIZE as u64, &hex(&body()), &mut quiet()).unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), body());
}

#[test]
fn cancelling_keeps_the_partial_file() {
    let ((base, _), dir) = (serve(), tempfile::tempdir().unwrap());
    let dest = dir.path().join("model.onnx");
    let url = format!("{base}/file");
    let cancelled =
        download(&[&url], &dest, SIZE as u64, &hex(&body()), &mut |done, _| done < 65536).unwrap_err();
    assert!(cancelled.is::<Cancelled>() && dest.with_added_extension("part").exists());
}
