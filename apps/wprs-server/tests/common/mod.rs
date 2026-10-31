#![allow(dead_code)]

use reqwest::header::{CONTENT_TYPE, SET_COOKIE};
use reqwest::{Client, Method, Response, StatusCode};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;
use wprs_format::{
    Content, Descriptor, Fetch, Limits, Manifest, Origin, Previews, Rating, SCHEMA, SCHEMA_V1,
};
use wprs_server::{App, Config};

mod server;

pub struct Server {
    pub app: Arc<App>,
    pub cfg: Config,
    pub url: String,
    pub dir: TempDir,
    pub http: Client,
}

pub enum Who {
    Nobody,
    Cookie(String),
    Bearer(String),
}

pub async fn start(extra: &[(&str, &str)]) -> Server {
    let dir = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let data = dir.path().join("data").display().to_string();
    let mut vars: HashMap<String, String> =
        [("WPRS_DATA", data), ("WPRS_PUBLIC_URL", url.clone()), ("WPRS_RATE_LIMITS", "false".into())]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
    vars.extend(extra.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    let mut cfg = Config::from_vars(|key| vars.get(key).cloned()).unwrap();
    cfg.validator = PathBuf::from(env!("CARGO_BIN_EXE_wprs-server"));
    let app = App::open(cfg.clone()).unwrap();
    tokio::spawn(app.clone().serve(listener));
    let http = Client::builder().redirect(reqwest::redirect::Policy::none()).build().unwrap();
    Server { app, cfg, url, dir, http }
}

pub fn cookie(response: &Response) -> String {
    let header = response.headers().get(SET_COOKIE).unwrap().to_str().unwrap();
    header.split(';').next().unwrap().split_once('=').unwrap().1.to_string()
}

pub fn content_type(response: &Response) -> String {
    response.headers().get(CONTENT_TYPE).map(|v| v.to_str().unwrap().to_string()).unwrap_or_default()
}

pub fn package(dir: &Path, id: &str, version: &str) -> PathBuf {
    build(
        dir,
        Manifest { schema: SCHEMA_V1.into(), preview: Some("preview.png".into()), ..manifest(id, version) },
    )
}

pub fn package_v2(dir: &Path, id: &str, version: &str, rating: Rating) -> PathBuf {
    let descriptors = if rating == Rating::Mature { vec![Descriptor::Sexual] } else { Vec::new() };
    let manifest = Manifest {
        schema: SCHEMA.into(),
        previews: Some(Previews { thumbnail: Some("preview.png".into()), animated: None }),
        content: Some(Content { rating, descriptors }),
        photosensitive: Some(true),
        origin: Some(Origin::default()),
        ..manifest(id, version)
    };
    build(dir, manifest)
}

fn manifest(id: &str, version: &str) -> Manifest {
    Manifest {
        id: id.into(),
        version: version.into(),
        title: format!("Dawn {version}").as_str().into(),
        description: "A quiet gradient".into(),
        authors: vec!["Example Studio".into()],
        license: "CC0-1.0".into(),
        kind: wprs_format::Kind::Image,
        entry: "image.png".into(),
        tags: vec!["sky".into()],
        ..Default::default()
    }
}

fn build(dir: &Path, manifest: Manifest) -> PathBuf {
    let src = dir.join(format!("src-{}-{}", manifest.id, manifest.version));
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("manifest.json"), manifest.to_json().unwrap()).unwrap();
    image::RgbImage::from_pixel(64, 32, image::Rgb([20, 60, 120])).save(src.join("image.png")).unwrap();
    image::RgbImage::from_pixel(32, 16, image::Rgb([200, 120, 40])).save(src.join("preview.png")).unwrap();
    let out = dir.join(format!("{}-{}.wpak", manifest.id, manifest.version));
    wprs_format::build(&src, &out, &Limits::default()).unwrap();
    out
}

pub async fn upload(server: &Server, who: &Who, wpak: &Path) -> Value {
    let bytes = fs::read(wpak).unwrap();
    let sha = wprs_format::sha256(&bytes[..]).unwrap();
    let (status, open) = server
        .json(Method::POST, "/api/v1/uploads", who, Some(json!({"size": bytes.len(), "sha256": sha})))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{open}");
    let id = open["id"].as_str().unwrap().to_string();
    let put =
        server.call(Method::PUT, &format!("/api/v1/uploads/{id}"), who).body(bytes).send().await.unwrap();
    assert_eq!(put.status(), StatusCode::OK);
    server.json(Method::POST, &format!("/api/v1/uploads/{id}/finalize"), who, None).await.1
}

pub struct HttpFetch;

impl Fetch for HttpFetch {
    fn get(&self, base: &str, path: &str, max: u64) -> io::Result<Option<Vec<u8>>> {
        let rest = base.strip_prefix("http://").ok_or_else(|| io::Error::other("http only"))?;
        let (host, prefix) = rest.split_once('/').unwrap_or((rest, ""));
        let mut stream = TcpStream::connect(host)?;
        write!(stream, "GET /{prefix}{path} HTTP/1.0\r\nHost: {host}\r\n\r\n")?;
        let mut reply = Vec::new();
        stream.take(max + 4096).read_to_end(&mut reply)?;
        let split =
            reply.windows(4).position(|w| w == b"\r\n\r\n").ok_or_else(|| io::Error::other("no header"))?;
        let head = String::from_utf8_lossy(&reply[..split]).to_string();
        match head.split(' ').nth(1) {
            Some("200") => Ok(Some(reply[split + 4..].to_vec())),
            Some("404" | "410") => Ok(None),
            other => Err(io::Error::other(format!("HTTP {other:?}"))),
        }
    }
}
