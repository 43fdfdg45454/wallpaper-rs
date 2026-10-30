use serde_json::json;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use wprs_core::dbus::{BUS_NAME, INTERFACE, OBJECT_PATH, error};
use zbus::Connection;

pub struct Bus {
    daemon: Child,
    address: String,
    _dir: tempfile::TempDir,
}

impl Bus {
    pub fn start() -> Bus {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("bus.conf");
        let policy = "<policy context=\"default\"><allow send_destination=\"*\"/><allow eavesdrop=\"true\"/>\
            <allow own=\"*\"/></policy>";
        let listen = format!("<listen>unix:dir={}</listen><auth>EXTERNAL</auth>", dir.path().display());
        std::fs::write(&config, format!("<busconfig>{listen}{policy}</busconfig>")).unwrap();
        let mut daemon = (Command::new("dbus-daemon").args(["--nofork", "--print-address", "--config-file"]))
            .arg(&config)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("dbus-daemon is installed");
        let mut address = String::new();
        BufReader::new(daemon.stdout.take().unwrap()).read_line(&mut address).unwrap();
        Bus { daemon, address: address.trim().into(), _dir: dir }
    }

    pub async fn connect(&self) -> Connection {
        zbus::connection::Builder::address(self.address.as_str()).build().await.unwrap()
    }

    pub async fn serve(&self, model: Option<u32>) -> Connection {
        let builder = zbus::connection::Builder::address(self.address.as_str()).name(BUS_NAME);
        let builder = match model {
            Some(model) => builder.serve_at(OBJECT_PATH, Daemon(model)).serve_at(OBJECT_PATH, Library),
            None => builder.serve_at(OBJECT_PATH, OldDaemon),
        };
        builder.build().await.unwrap()
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        self.daemon.kill().ok();
        self.daemon.wait().ok();
    }
}

struct Daemon(u32);

#[zbus::interface(name = "io.github.wallpaper_rs.Daemon1")]
impl Daemon {
    #[zbus(property(emits_changed_signal = "const"))]
    fn model(&self) -> u32 {
        self.0
    }

    fn get_job(&self, job: u64) -> String {
        let job = json!({"id": job, "kind": "scan", "target": "", "state": "done", "done": 2, "total": 2,
            "started_at": 1, "finished_at": 2, "error": null, "output": {"reports": []}});
        json!({"model": self.0, "serial": 9, "job": job}).to_string()
    }
}

struct OldDaemon;

#[zbus::interface(name = "io.github.wallpaper_rs.Daemon1")]
impl OldDaemon {
    fn get_state(&self) -> String {
        "{}".into()
    }
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "io.github.wallpaper_rs.Error")]
enum MockError {
    #[zbus(error)]
    ZBus(zbus::Error),
    NotFound(String),
}

struct Library;

#[zbus::interface(name = "io.github.wallpaper_rs.Library1")]
impl Library {
    async fn rescan(&self, source: &str, #[zbus(connection)] bus: &Connection) -> Result<u64, MockError> {
        let bus = bus.clone();
        match source {
            "fast" => {
                for (job, done) in [(9, 1), (1, 1), (1, 2)] {
                    progress(&bus, job, done).await;
                }
                finished(&bus, 9, "failed", error::STALE).await;
                finished(&bus, 1, "done", "").await;
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok(1)
            }
            "late" | "stuck" => {
                let job = if source == "late" { 2 } else { 3 };
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    progress(&bus, job, 3).await;
                    if job == 2 {
                        finished(&bus, job, "failed", error::STALE).await;
                    }
                });
                Ok(job)
            }
            "silent" => {
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    let body = (4_u64, "failed", "", "");
                    bus.emit_signal(None::<&str>, OBJECT_PATH, INTERFACE, "JobFinished", &body)
                        .await
                        .unwrap();
                });
                Ok(4)
            }
            _ => Err(MockError::NotFound(format!("no source {source}"))),
        }
    }
}

async fn progress(bus: &Connection, job: u64, done: u64) {
    bus.emit_signal(None::<&str>, OBJECT_PATH, INTERFACE, "JobProgress", &(job, done, 4_u64)).await.unwrap();
}

async fn finished(bus: &Connection, job: u64, state: &str, name: &str) {
    let body = (job, state, name, if name.is_empty() { "" } else { "the signed index expired" });
    bus.emit_signal(None::<&str>, OBJECT_PATH, INTERFACE, "JobFinished", &body).await.unwrap();
}
