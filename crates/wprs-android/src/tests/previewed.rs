use crate::repos;
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use wprs_format::{
    Kind, Limits, Manifest, SCHEMA_V1, Settings, Signer, Text, add, build, init, keygen, publish, sign_root,
};

const NOW: i64 = 1_800_000_000;
const WEBP: &[u8] = b"RIFF\0\0\0\0WEBP";

fn serve(dir: PathBuf) -> (u16, Arc<AtomicU32>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let (port, hits) = (listener.local_addr().unwrap().port(), Arc::new(AtomicU32::new(0)));
    let counted = hits.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let mut head = [0u8; 2048];
            let n = stream.read(&mut head).unwrap_or(0);
            let text = String::from_utf8_lossy(&head[..n]).into_owned();
            let path = text
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .trim_start_matches('/')
                .split('?')
                .next()
                .unwrap_or("")
                .to_owned();
            if path.starts_with("previews/") {
                counted.fetch_add(1, Relaxed);
            }
            let body = fs::read(dir.join(path));
            let reply = match &body {
                Ok(bytes) => {
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len())
                }
                Err(_) => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
            };
            let _ = stream.write_all(reply.as_bytes());
            let _ = stream.write_all(body.as_deref().unwrap_or_default());
        }
    });
    (port, hits)
}

fn package(dir: &Path, id: &str) -> PathBuf {
    let src = dir.join(format!("src-{id}"));
    fs::create_dir_all(&src).unwrap();
    let manifest = Manifest {
        schema: SCHEMA_V1.into(),
        id: id.into(),
        version: "1.0.0".into(),
        title: Text::Plain("Sky".into()),
        description: "A calm gradient".into(),
        license: "CC0-1.0".into(),
        kind: Kind::Image,
        entry: "image.png".into(),
        preview: Some("preview.webp".into()),
        ..Default::default()
    };
    fs::write(src.join("manifest.json"), manifest.to_json().unwrap()).unwrap();
    fs::write(src.join("image.png"), id).unwrap();
    fs::write(src.join("preview.webp"), WEBP).unwrap();
    let out = dir.join(format!("{id}.wpak"));
    build(&src, &out, &Limits::default()).unwrap();
    out
}

pub(super) fn repository_previews_come_from_that_repository_once_and_within_a_bound() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    let (root, online) = (keygen(None).unwrap(), keygen(None).unwrap());
    let settings = Settings {
        name: "Example".into(),
        root_keys: vec![root.public.clone()],
        online_keys: vec![online.public.clone()],
        ..Settings::default()
    };
    let now = crate::library::now();
    init(&repo, &settings, now).unwrap();
    sign_root(&repo, &Signer::new(&root.secret, None).unwrap()).unwrap();
    add(&repo, &package(tmp.path(), "org.example.sky"), None, now).unwrap();
    publish(&repo, &Signer::new(&online.secret, None).unwrap(), 7, now).unwrap();
    let (port, hits) = serve(repo);
    let added = repos::add(&format!("http://127.0.0.1:{port}/?fingerprint={}", root.fingerprint)).unwrap();
    let repo_id = added["repo_id"].as_str().unwrap().to_owned();
    let listed = repos::browse("").unwrap();
    assert_eq!(listed[0]["version"], "1.0.0", "{listed}");
    let spec = format!("{repo_id}/org.example.sky/1.0.0");
    let path = repos::preview::thumb(&spec).unwrap();
    let path = PathBuf::from(path.as_str().unwrap_or_else(|| panic!("{path}")));
    assert_eq!(fs::read(&path).unwrap(), WEBP);
    assert_eq!(hits.load(Relaxed), 1);
    assert_eq!(repos::preview::thumb(&spec).unwrap(), Value::from(path.to_str().unwrap()));
    assert_eq!(hits.load(Relaxed), 1, "the second look is served from the cache");
    assert!(repos::preview::thumb("nothing/org.example.sky/1.0.0").is_err());
    assert!(repos::preview::thumb("garbled").unwrap().is_null());
    repos::preview::evict(path.parent().unwrap(), 0);
    assert!(!path.exists(), "the cache keeps to its bound");
    repos::preview::thumb(&spec).unwrap();
    assert!(path.is_file());
    repos::remove(&repo_id).unwrap();
    assert!(!path.exists(), "removing a repository forgets its previews");
}
