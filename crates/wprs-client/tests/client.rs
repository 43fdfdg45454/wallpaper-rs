use serde_json::json;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use wprs_client::{
    Client, Daemon1Proxy, Library1Proxy, Playlists1Proxy, Profiles1Proxy, Providers1Proxy, Repos1Proxy,
    error_name, explain, parse,
};
use wprs_core::dbus::{
    BUS_NAME, INTERFACE, LIBRARY, OBJECT_PATH, PLAYLISTS, PROFILES, PROVIDERS, REPOS, error,
};
use wprs_core::model::{JobState, TagsReply};
use zbus::Connection;
use zbus::proxy::Defaults;

struct Bus {
    daemon: Child,
    address: String,
    _dir: tempfile::TempDir,
}

impl Bus {
    fn start() -> Bus {
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

    async fn connect(&self) -> Connection {
        zbus::connection::Builder::address(self.address.as_str()).build().await.unwrap()
    }

    async fn serve(&self, model: Option<u32>) -> Connection {
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

async fn within<T>(work: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), work).await.expect("the job ends in time")
}

#[tokio::test]
async fn a_job_that_finishes_before_the_reply_is_not_missed() {
    let bus = Bus::start();
    let _daemon = bus.serve(Some(1)).await;
    let client = Client::new(bus.connect().await).await.unwrap();
    let mut seen = Vec::new();
    let rescan = async { Ok(((), client.library.rescan("fast").await?)) };
    let ((), job) = within(client.run_job(rescan, |done, total| seen.push((done, total)))).await.unwrap();
    assert_eq!((job.id, job.state, job.output), (1, JobState::Done, Some(json!({"reports": []}))));
    assert_eq!(seen, [(1, 4), (2, 4)]);
}

#[tokio::test]
async fn failed_jobs_and_calls_keep_their_error_names() {
    let bus = Bus::start();
    let _daemon = bus.serve(Some(1)).await;
    let client = Client::new(bus.connect().await).await.unwrap();
    let mut seen = Vec::new();
    let late = async { client.library.rescan("late").await.map(|job| ((), job)) };
    let failed = within(client.run_job(late, |done, total| seen.push((done, total)))).await.unwrap_err();
    assert_eq!(
        (error_name(&failed), failed.to_string()),
        (Some(error::STALE), "the signed index expired".into())
    );
    assert_eq!(seen, [(3, 4)]);
    let missing = async { client.library.rescan("gone").await.map(|job| ((), job)) };
    let missing = client.run_job(missing, |_, _| {}).await.unwrap_err();
    assert_eq!(
        (error_name(&missing), missing.to_string()),
        (Some(error::NOT_FOUND), "no source gone".into())
    );
}

#[tokio::test]
async fn a_daemon_restart_while_waiting_fails_the_job() {
    let bus = Bus::start();
    let daemon = bus.serve(Some(1)).await;
    let client = Client::new(bus.connect().await).await.unwrap();
    let (running, mut started) = tokio::sync::mpsc::unbounded_channel();
    let stuck = async { client.library.rescan("stuck").await.map(|job| ((), job)) };
    let job = client.run_job(stuck, |_, _| running.send(()).unwrap());
    let restart = async {
        started.recv().await;
        daemon.close().await.unwrap();
        bus.serve(Some(1)).await
    };
    let (failed, _restarted) = within(async { tokio::join!(job, restart) }).await;
    assert_eq!(failed.unwrap_err().to_string(), "wprsd restarted while job 3 was running");
}

#[tokio::test]
async fn the_data_model_is_checked_when_connecting() {
    let bus = Bus::start();
    let absent = Client::new(bus.connect().await).await.unwrap_err().to_string();
    assert_eq!(absent, format!("wprsd is not running ({BUS_NAME} is not on the session bus)"));
    let hint = "but this client uses 1; install matching versions of wallpaper-rs";
    for (model, expected) in [(Some(2), 2), (None, 0)] {
        let daemon = bus.serve(model).await;
        let error = Client::new(bus.connect().await).await.unwrap_err();
        assert_eq!(error.to_string(), format!("wprsd uses data model {expected} {hint}"));
        daemon.close().await.unwrap();
    }
    let daemon = bus.serve(Some(1)).await;
    let client = Client::new(bus.connect().await).await.unwrap();
    let older = client.repos.list().await.map_err(explain).unwrap_err();
    assert_eq!(
        older.to_string(),
        "wprsd is older than this client; install matching versions of wallpaper-rs"
    );
    assert_eq!(error_name(&older), Some("org.freedesktop.DBus.Error.UnknownInterface"));
    daemon.close().await.unwrap();
}

#[test]
fn replies_are_parsed_after_their_data_model_is_checked() {
    let tags: TagsReply =
        parse(r#"{"model": 1, "serial": 5, "tags": [{"name": "sea", "count": 2}]}"#).unwrap();
    assert_eq!((tags.serial, tags.tags[0].count), (5, 2));
    let newer = parse::<TagsReply>(r#"{"model": 2, "serial": 5, "tags": []}"#).unwrap_err().to_string();
    assert!(newer.starts_with("wprsd uses data model 2 but this client uses 1"), "{newer}");
    assert!(parse::<TagsReply>(r#"{"serial": 5}"#).unwrap_err().to_string().contains("data model 0"));
}

fn names<P: Defaults>() -> [Option<&'static str>; 3] {
    [
        P::INTERFACE.as_ref().map(|i| i.as_str()),
        P::DESTINATION.as_ref().map(|d| d.as_str()),
        P::PATH.as_ref().map(|p| p.as_str()),
    ]
}

#[test]
fn every_proxy_uses_the_core_names() {
    let proxies = [names::<Daemon1Proxy>(), names::<Library1Proxy>(), names::<Playlists1Proxy>()]
        .into_iter()
        .chain([names::<Profiles1Proxy>(), names::<Providers1Proxy>(), names::<Repos1Proxy>()]);
    let interfaces = [INTERFACE, LIBRARY, PLAYLISTS, PROFILES, PROVIDERS, REPOS];
    for (proxy, interface) in proxies.zip(interfaces) {
        assert_eq!(proxy, [Some(interface), Some(BUS_NAME), Some(OBJECT_PATH)]);
    }
}
