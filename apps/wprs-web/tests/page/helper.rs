use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use wprs_web::{Allowed, Folder, FromWeb, Isolation, Link, Settings, Start, ToWeb, spawn};

pub const HELPER: &str = env!("CARGO_BIN_EXE_wprs-web");

pub fn available() -> bool {
    let ok = std::process::Command::new(HELPER).arg("--check").status().is_ok_and(|s| s.success());
    if !ok && std::env::var_os("WPRS_REQUIRE_WEB").is_some_and(|v| v == "1") {
        panic!("WPRS_REQUIRE_WEB=1 but WPE WebKit 2.54 is not available");
    }
    ok
}

pub struct Page {
    pub link: Link,
    pub child: std::process::Child,
    pub said: BTreeMap<String, Vec<Value>>,
    pub frames: u32,
    pub crashed: u32,
    pub broken: bool,
}

impl Page {
    pub fn open(start: Start, isolation: Isolation) -> Page {
        let isolation = Isolation { devices: isolation.devices || start.device.is_none(), ..isolation };
        let (link, child) = spawn(Path::new(HELPER), isolation, |c| {
            c.env_remove("WPRS_LOG");
        })
        .unwrap();
        let properties = start.properties.clone();
        link.send(&ToWeb::Start(Box::new(start)), &[]).unwrap();
        link.send(&ToWeb::Open { view: 1, size: (320, 180), scale: 1.0, properties }, &[]).unwrap();
        Page { link, child, said: BTreeMap::new(), frames: 0, crashed: 0, broken: false }
    }

    pub fn pump(&mut self, until: Instant) {
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
            FromWeb::Crashed { .. } => self.crashed += 1,
            FromWeb::Broken { .. } => self.broken = true,
            FromWeb::Failed(reason) | FromWeb::Unavailable(reason) => panic!("{reason}"),
            _ => {}
        }
    }

    pub fn wait(&mut self, name: &str, seconds: u64) -> Value {
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

pub fn package(dir: &Path, extra: &[(&str, &[u8])]) -> PathBuf {
    let source = dir.join("source");
    std::fs::create_dir_all(&source).unwrap();
    for entry in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/probe")).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), source.join(entry.file_name())).unwrap();
    }
    extra.iter().for_each(|(name, bytes)| std::fs::write(source.join(name), bytes).unwrap());
    let out = dir.join("probe.wpak");
    wprs_format::build(&source, &out, &wprs_format::Limits::default()).unwrap();
    out
}

fn node() -> Option<String> {
    let mut nodes: Vec<String> = std::fs::read_dir("/dev/dri")
        .ok()?
        .flatten()
        .map(|e| e.path().to_string_lossy().into_owned())
        .collect();
    nodes.sort();
    nodes.into_iter().find(|n| n.contains("renderD"))
}

pub fn start(dir: &Path, port: u16) -> Start {
    with(dir, port, &[])
}

pub fn with(dir: &Path, port: u16, extra: &[(&str, &[u8])]) -> Start {
    let picture = dir.join("photo.png");
    std::fs::write(&picture, b"\x89PNG\r\n\x1a\n").unwrap();
    let token = wprs_web::token("photos", &[7; 32], &picture);
    let properties = json!({"tint": "#ff8000", "speed": 1, "port": port}).as_object().unwrap().clone();
    Start {
        package: package(dir, extra),
        id: "org.example.probe".into(),
        entry: "index.html".into(),
        preload: Some("early.js".into()),
        storage: Some(dir.join("storage")),
        device: wprs_web::device(Path::new(HELPER), node()),
        languages: vec!["es-AR".into()],
        properties,
        shape: json!({"kinds": {"tint": "color", "speed": "slider"}}),
        settings: Settings { fps: 20, quality: 3, locale: "es".into(), volume: 0.0 },
        allowed: Allowed { audio: true, pointer: true, clicks: true },
        folders: vec![Folder { prop: "photos".into(), all: true, files: vec![(token, picture)] }],
        ..Start::default()
    }
}
