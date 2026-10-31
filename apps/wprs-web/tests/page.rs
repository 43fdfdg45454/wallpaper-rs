use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use wprs_web::{Allowed, Folder, FromWeb, Isolation, Link, Settings, Start, ToWeb, spawn};

const HELPER: &str = env!("CARGO_BIN_EXE_wprs-web");

fn available() -> bool {
    let ok = std::process::Command::new(HELPER).arg("--check").status().is_ok_and(|s| s.success());
    if !ok && std::env::var_os("WPRS_REQUIRE_WEB").is_some_and(|v| v == "1") {
        panic!("WPRS_REQUIRE_WEB=1 but WPE WebKit 2.54 is not available");
    }
    ok
}

struct Page {
    link: Link,
    child: std::process::Child,
    said: BTreeMap<String, Vec<Value>>,
    frames: u32,
}

impl Page {
    fn open(start: Start, isolation: Isolation) -> Page {
        let (link, child) = spawn(Path::new(HELPER), isolation, |c| {
            c.env_remove("WPRS_LOG");
        })
        .unwrap();
        let properties = start.properties.clone();
        link.send(&ToWeb::Start(Box::new(start)), &[]).unwrap();
        link.send(&ToWeb::Open { view: 1, size: (320, 180), scale: 1.0, properties }, &[]).unwrap();
        Page { link, child, said: BTreeMap::new(), frames: 0 }
    }

    fn pump(&mut self, until: Instant) {
        while Instant::now() < until {
            match self.link.recv::<FromWeb>(false) {
                Ok(Some((message, _))) => self.take(message),
                Ok(None) => panic!("the helper stopped"),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(e) => panic!("{e}"),
            }
        }
    }

    fn take(&mut self, message: FromWeb) {
        match message {
            FromWeb::Frame { view, buffer } => {
                self.frames += 1;
                self.link.send(&ToWeb::Released { view, buffer }, &[]).unwrap();
            }
            FromWeb::Pixels { view, slot, .. } => {
                self.frames += 1;
                self.link.send(&ToWeb::Freed { view, slot }, &[]).unwrap();
            }
            FromWeb::Said(text) => {
                let Some(rest) = text.strip_prefix("log: probe ") else { return };
                let (name, value) = rest.split_once(' ').unwrap();
                self.said.entry(name.into()).or_default().push(serde_json::from_str(value).unwrap());
            }
            FromWeb::Failed(reason) | FromWeb::Unavailable(reason) => panic!("{reason}"),
            _ => {}
        }
    }

    fn wait(&mut self, name: &str, seconds: u64) -> Value {
        let end = Instant::now() + Duration::from_secs(seconds);
        while Instant::now() < end && !self.said.contains_key(name) {
            self.pump(Instant::now() + Duration::from_millis(50));
        }
        self.said
            .get(name)
            .and_then(|v| v.last())
            .cloned()
            .unwrap_or_else(|| panic!("no {name}: {:?}", self.said))
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        let _ = self.link.send(&ToWeb::Quit, &[]);
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn package(dir: &Path) -> PathBuf {
    let out = dir.join("probe.wpak");
    wprs_format::build(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/probe"),
        &out,
        &wprs_format::Limits::default(),
    )
    .unwrap();
    out
}

fn start(dir: &Path, port: u16) -> Start {
    let picture = dir.join("photo.png");
    std::fs::write(&picture, b"\x89PNG\r\n\x1a\n").unwrap();
    let token = wprs_web::token("photos", &[7; 32], &picture);
    let properties = json!({"tint": "#ff8000", "speed": 1, "port": port}).as_object().unwrap().clone();
    Start {
        package: package(dir),
        id: "org.example.probe".into(),
        entry: "index.html".into(),
        preload: Some("early.js".into()),
        storage: Some(dir.join("storage")),
        device: std::fs::read_dir("/dev/dri").ok().and_then(|d| {
            let mut nodes: Vec<String> =
                d.flatten().map(|e| e.path().to_string_lossy().into_owned()).collect();
            nodes.sort();
            nodes.into_iter().find(|n| n.contains("renderD"))
        }),
        languages: vec!["es-AR".into()],
        properties,
        shape: json!({"kinds": {"tint": "color", "speed": "slider"}}),
        settings: Settings { fps: 20, quality: 3, locale: "es".into(), volume: 0.0 },
        allowed: Allowed { audio: true, pointer: true, clicks: true },
        folders: vec![Folder { prop: "photos".into(), all: true, files: vec![(token, picture)] }],
        ..Start::default()
    }
}

#[test]
fn page_gets_its_api_and_nothing_else() {
    if !available() {
        return eprintln!("skipped: no WPE WebKit 2.54");
    }
    let dir = tempfile::tempdir().unwrap();
    let server = TcpListener::bind("127.0.0.1:0").unwrap();
    server.set_nonblocking(true).unwrap();
    let port = server.local_addr().unwrap().port();
    let mut page = Page::open(start(dir.path(), port), Isolation { network: true, devices: false });
    assert_eq!(page.wait("early", 20), "preloaded");
    assert_eq!(page.wait("language", 5), "es-AR");
    assert_eq!(page.wait("properties", 5)["tint"]["hex"], "#ff8000");
    assert_eq!(page.wait("settings", 5)["fps"], 20);
    for name in ["manifest", "other", "file", "escape", "user", "local"] {
        assert_eq!(page.wait(name, 10), "blocked", "{name}");
    }
    assert_eq!(page.wait("own", 5).as_str().map(|s| s.starts_with("read")), Some(true));
    assert_eq!(page.wait("picked", 5), "read 8");
    assert_eq!(page.wait("files", 5).as_array().map(Vec::len), Some(1));
    assert_eq!(page.wait("visits", 5), "1");
    assert!(server.accept().is_err(), "the page reached a local server");
    let values = json!({"speed": 2}).as_object().unwrap().clone();
    page.link.send(&ToWeb::Properties { view: 1, values }, &[]).unwrap();
    page.said.remove("properties");
    assert_eq!(page.wait("properties", 5), json!({"speed": 2}));
    page.said.remove("frames");
    page.wait("frames", 5);
    let rate = page.wait("frames", 5).as_u64().unwrap();
    assert!((10..=26).contains(&rate), "{rate} frames a second at a cap of 20");
    page.link.send(&ToWeb::Visible { view: 1, visible: false }, &[]).unwrap();
    assert_eq!(page.wait("paused", 5), true);
    page.frames = 0;
    page.pump(Instant::now() + Duration::from_secs(2));
    assert!(page.frames <= 2, "{} frames while hidden", page.frames);
    let required = std::env::var_os("WPRS_REQUIRE_WEB").is_some_and(|v| v == "1");
    assert!(!required || wprs_web::isolated(page.child.id()), "the page shares the network of the daemon");
}

#[test]
fn page_without_a_render_node_draws_in_memory() {
    if !available() {
        return eprintln!("skipped: no WPE WebKit 2.54");
    }
    let dir = tempfile::tempdir().unwrap();
    let start = Start { device: None, storage: None, mask_gpu: true, ..start(dir.path(), 9) };
    let mut page = Page::open(start, Isolation { network: true, devices: true });
    assert_eq!(page.wait("visits", 20), "1");
    let webgl = page.wait("webgl", 5);
    assert!(webgl.is_null() || webgl == "WebKit WebGL", "{webgl}");
    page.frames = 0;
    page.pump(Instant::now() + Duration::from_secs(1));
    assert!(page.frames >= 5, "{} frames in memory", page.frames);
}
