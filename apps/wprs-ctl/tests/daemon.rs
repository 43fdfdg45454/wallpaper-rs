use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wprs_core::dbus::{BUS_NAME, INTERFACE, OBJECT_PATH};
use zbus::Connection;

const UIDS: [&str; 5] = [
    "01J9Z0X9Z6Q7X4F3K5E1V0R1AB",
    "01J9Z0X9Z6Q7X4F3K5E1V0R2CD",
    "01J9Z0X9Z6Q7X4F3K5E1V0R3EF",
    "01J9Z0X9Z6Q7X4F3K5E1V0R4GH",
    "01J9Z0X9Z6Q7X4F3K5E1V0R5JK",
];

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<String>>>);

impl Log {
    fn push(&self, call: String) {
        self.0.lock().unwrap().push(call);
    }

    fn has(&self, prefix: &str) -> bool {
        self.0.lock().unwrap().iter().any(|call| call.starts_with(prefix))
    }
}

struct Bus {
    daemon: Child,
    address: String,
    log: Log,
    _service: Connection,
    _dir: tempfile::TempDir,
}

impl Bus {
    async fn start() -> Bus {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("bus.conf");
        let policy = "<policy context=\"default\"><allow send_destination=\"*\"/><allow eavesdrop=\"true\"/><allow own=\"*\"/></policy>";
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
        let (address, log) = (address.trim().to_string(), Log::default());
        let service = zbus::connection::Builder::address(address.as_str()).name(BUS_NAME);
        let service =
            service.serve_at(OBJECT_PATH, Daemon(log.clone())).serve_at(OBJECT_PATH, Library(log.clone()));
        let service =
            service.serve_at(OBJECT_PATH, Providers(log.clone())).serve_at(OBJECT_PATH, Repos(log.clone()));
        let service = service.serve_at(OBJECT_PATH, Playlists(log.clone()));
        Bus { daemon, address, log, _service: service.build().await.unwrap(), _dir: dir }
    }

    fn spawn(&self, args: &[&str], stdin: &str) -> Child {
        let mut child = Command::new(env!("CARGO_BIN_EXE_wprs-ctl"))
            .args(args)
            .env("DBUS_SESSION_BUS_ADDRESS", &self.address)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
        child
    }

    fn ctl(&self, args: &[&str]) -> (i32, String, String) {
        self.fed(args, "")
    }

    fn fed(&self, args: &[&str], stdin: &str) -> (i32, String, String) {
        texts(self.spawn(args, stdin).wait_with_output().unwrap())
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        self.daemon.kill().ok();
        self.daemon.wait().ok();
    }
}

fn texts(out: Output) -> (i32, String, String) {
    let text = |bytes: Vec<u8>| String::from_utf8(bytes).unwrap();
    (out.status.code().unwrap_or(-1), text(out.stdout), text(out.stderr))
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "io.github.wallpaper_rs.Error")]
enum Failure {
    #[zbus(error)]
    ZBus(zbus::Error),
    NotFound(String),
    Stale(String),
}

struct Daemon(Log);

#[zbus::interface(name = "io.github.wallpaper_rs.Daemon1")]
impl Daemon {
    #[zbus(property(emits_changed_signal = "const"))]
    fn model(&self) -> u32 {
        1
    }

    fn get_job(&self, job: u64) -> String {
        let state = if self.0.has(&format!("CancelJob {job}")) { "cancelled" } else { "done" };
        let output =
            json!({"reports": [{"source": null, "added": 2, "updated": 0, "moved": 0, "missing": 0}]});
        let job = json!({"id": job, "kind": "scan", "target": "", "state": state, "done": 2, "total": 2,
            "started_at": 1, "finished_at": 2, "error": null, "output": output});
        json!({"model": 1, "serial": 9, "job": job}).to_string()
    }

    async fn cancel_job(&self, job: u64, #[zbus(connection)] bus: &Connection) {
        self.0.push(format!("CancelJob {job}"));
        signal(bus, "JobFinished", &(job, "cancelled", "", "cancelled")).await;
    }

    fn get_schema(&self) -> String {
        let immich = json!({"kind": "immich", "hosts": [], "options": [],
            "secret": {"name": "api_key", "required": true}});
        json!({"model": 1, "serial": 9, "options": [], "rules": [], "auto": [], "preset": [], "providers": [immich],
            "query": [], "thumb_sizes": [256, 512, 1024], "fits": ["fill"]})
        .to_string()
    }
}

struct Library(Log);

#[zbus::interface(name = "io.github.wallpaper_rs.Library1")]
impl Library {
    fn resolve(&self, refs: Vec<String>) -> Result<Vec<String>, Failure> {
        self.0.push(format!("Resolve {}", refs.join(" ")));
        let find =
            |r: &String| UIDS.iter().find(|uid| uid.ends_with(&r.to_uppercase())).map(|uid| uid.to_string());
        let found =
            refs.iter().map(|r| find(r).ok_or_else(|| Failure::NotFound(format!("no item matches \"{r}\""))));
        found.collect()
    }

    async fn rescan(&self, source: String, #[zbus(connection)] bus: &Connection) -> u64 {
        self.0.push(format!("Rescan {source}"));
        let (bus, log, job) = (bus.clone(), self.0.clone(), if source == "slow" { 7 } else { 1 });
        tokio::spawn(async move {
            let mut done = 0_u64;
            while !log.has(&format!("CancelJob {job}")) {
                tokio::time::sleep(Duration::from_millis(50)).await;
                done = (done + 1).min(2);
                signal(&bus, "JobProgress", &(job, done, 2_u64)).await;
                if job == 1 && done == 2 {
                    return signal(&bus, "JobFinished", &(job, "done", "", "")).await;
                }
            }
        });
        job
    }

    fn search(&self, query: String, offset: u32, limit: u32) -> String {
        self.0.push(format!("Search {query} {offset} {limit}"));
        let item = |uid: &&str| {
            json!({"uid": uid, "kind": "image", "title": format!("Sea {}", &uid[20..]),
            "width": 64, "height": 32, "duration_ms": null, "color": null, "tags": ["sea"], "rating": 0,
            "favorite": false, "missing": false, "error": null, "revoked": null, "removable": true,
            "imported": false, "thumb": null})
        };
        let items: Vec<_> = UIDS.iter().skip(offset as usize).take(limit as usize).map(item).collect();
        let seeded = query.split(' ').map(|word| if word == "sort:random" { "sort:random:7" } else { word });
        let query = seeded.collect::<Vec<_>>().join(" ");
        json!({"model": 1, "serial": 9, "query": query, "total": UIDS.len(), "offset": offset, "items": items})
            .to_string()
    }
}

struct Playlists(Log);

#[zbus::interface(name = "io.github.wallpaper_rs.Playlists1")]
impl Playlists {
    fn get(&self, playlist: String, offset: u32, limit: u32) -> String {
        self.0.push(format!("Get {playlist} {offset} {limit}"));
        let rules = json!({"order": "sequential", "interval": "30m", "at": [], "days": [], "synced": false,
            "resume": true, "advance_while_paused": false, "skip_videos_on_battery": false, "transition_ms": null});
        let info = json!({"uid": UIDS[4], "name": playlist, "query": null, "rules": rules, "count": 600,
            "outputs": []});
        let item = |i: u32| {
            json!({"uid": UIDS[i as usize % 5], "kind": "image", "title": "Sea", "width": null, "height": null,
            "duration_ms": null, "color": null, "tags": [], "rating": 0, "favorite": false, "missing": false,
            "error": null, "revoked": null, "removable": true, "imported": false, "thumb": null})
        };
        let items: Vec<_> = (offset..600.min(offset.saturating_add(limit))).map(item).collect();
        json!({"model": 1, "serial": 9, "playlist": info, "total": 600, "offset": offset, "items": items})
            .to_string()
    }

    fn remove_entries(&self, playlist: String, indexes: Vec<u32>, expected: Vec<String>) {
        self.0.push(format!("RemoveEntries {playlist} {indexes:?} {expected:?}"));
    }
}

struct Providers(Log);

#[zbus::interface(name = "io.github.wallpaper_rs.Providers1")]
impl Providers {
    fn add(&self, id: String, kind: String, options: HashMap<String, String>, secret: String) -> String {
        let options: BTreeMap<_, _> = options.into_iter().collect();
        self.0.push(format!("Add {id} {kind} {options:?} [{secret}]"));
        UIDS[0].into()
    }
}

struct Repos(Log);

#[zbus::interface(name = "io.github.wallpaper_rs.Repos1")]
impl Repos {
    fn install(&self, spec: String, allow_stale: bool, allow_downgrade: bool) -> Result<u64, Failure> {
        self.0.push(format!("Install {spec} {allow_stale} {allow_downgrade}"));
        match allow_stale {
            true => Ok(5),
            false => Err(Failure::Stale("repository 01234567 is stale: its signed index has expired".into())),
        }
    }

    fn list_installed(&self) -> String {
        let package = |repo: &str, id: &str| {
            json!({"repo_id": repo, "id": id, "version": "1.0.0", "sha256": "ab", "size": 1, "signer": null,
            "revoked": null, "item": null, "update": "1.1.0"})
        };
        let packages = [
            package("0123456789abcdef", "org.example.dawn"),
            package("fedcba9876543210", "org.example.dusk"),
        ];
        json!({"model": 1, "serial": 9, "packages": packages}).to_string()
    }

    fn browse(&self, query: String, offset: u32, limit: u32) -> String {
        self.0.push(format!("Browse {query} {offset} {limit}"));
        let package = json!({"repo_id": "0123456789abcdef", "repo_name": "Example wallpapers", "id": query,
            "author": null, "version": "1.0.0", "kind": "image", "title": "Dawn", "description": "", "tags": [],
            "license": "CC0-1.0", "authors": [], "size": 1, "published": 0, "preview": false,
            "versions": ["1.0.0"], "installed": [], "update": false});
        json!({"model": 1, "serial": 9, "query": query, "total": 1, "offset": 0, "packages": [package]})
            .to_string()
    }
}

async fn signal<B: serde::Serialize + zbus::zvariant::DynamicType>(bus: &Connection, name: &str, body: &B) {
    bus.emit_signal(None::<&str>, OBJECT_PATH, INTERFACE, name, body).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_waited_job_shows_its_result() {
    let bus = Bus::start().await;
    assert_eq!(bus.ctl(&["lib", "scan"]), (0, "1\n".into(), String::new()));
    let (code, out, _) = bus.ctl(&["lib", "scan", "--wait"]);
    assert_eq!((code, out.as_str()), (0, "job 1 scan done: 2 added, 0 missing, 0 moved, 0 updated\n"));
    let (code, out, _) = bus.ctl(&["lib", "scan", "--wait", "--json"]);
    let job: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!((code, job["id"].as_u64(), job["state"].as_str()), (0, Some(1), Some("done")));
}

#[tokio::test(flavor = "multi_thread")]
async fn ctrl_c_stops_waiting_and_cancel_stops_the_job() {
    let bus = Bus::start().await;
    let waiting = bus.spawn(&["lib", "scan", "slow", "--wait"], "");
    let start = Instant::now();
    while !bus.log.has("Rescan slow") {
        assert!(start.elapsed() < Duration::from_secs(10), "the scan never started");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    Command::new("kill").args(["-INT", &waiting.id().to_string()]).status().unwrap();
    let (code, out, err) = texts(waiting.wait_with_output().unwrap());
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(err.contains("stopped waiting; job 7 keeps running (wprs-ctl cancel 7 stops it)"), "{err}");
    assert!(!bus.log.has("CancelJob"));
    assert_eq!(bus.ctl(&["cancel", "7"]).0, 0);
    assert!(bus.log.has("CancelJob 7"));
    let (code, out, _) = bus.ctl(&["job", "7"]);
    assert!(code == 0 && out.contains("state = cancelled\n"), "{out}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_search_page_prints_the_normalized_query() {
    let bus = Bus::start().await;
    let (code, out, err) =
        bus.ctl(&["lib", "search", "tag:sea sort:random", "--offset", "2", "--limit", "2"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        out,
        "query = tag:sea sort:random:7\nitems = 3-4 of 5\n\
         ITEM      KIND   SIZE   RATING  FLAGS  TITLE       TAGS\n\
         E1V0R3EF  image  64x32  0              Sea V0R3EF  sea\n\
         E1V0R4GH  image  64x32  0              Sea V0R4GH  sea\n"
    );
    assert!(bus.log.has("Search tag:sea sort:random 2 2"));
    let (_, out, _) = bus.ctl(&["lib", "search", "tag:sea sort:random:7", "--limit", "1", "--json"]);
    let page: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        (page["query"].as_str(), page["items"][0]["uid"].as_str()),
        (Some("tag:sea sort:random:7"), Some(UIDS[0]))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn daemon_errors_map_to_messages_and_exit_codes() {
    let bus = Bus::start().await;
    let (code, _, err) = bus.ctl(&["lib", "show", "nosuch"]);
    assert_eq!((code, err.as_str()), (1, "Error: no item matches \"nosuch\"\n"));
    let (code, _, err) = bus.ctl(&["lib", "show", "https://example.org/a.png"]);
    assert_eq!((code, err.as_str()), (2, "Error: https://example.org/a.png is not a local file\n"));
    assert!(!bus.log.has("Resolve https"));
    let (code, _, err) = bus.ctl(&["pkg", "install", "org.example.dawn"]);
    assert_eq!(
        (code, err.as_str()),
        (1, "Error: repository 01234567 is stale: its signed index has expired\n")
    );
    assert!(bus.log.has("Browse org.example.dawn 0 500") && !bus.log.has("Install org.example.dawn true"));
    assert_eq!(
        bus.ctl(&["pkg", "install", "org.example.dawn", "--allow-stale"]),
        (0, "5\n".into(), String::new())
    );
    let (code, _, err) = bus.ctl(&["profile", "ls"]);
    let older = "Error: wprsd is older than this client; install matching versions of wallpaper-rs\n";
    assert_eq!((code, err.as_str()), (1, older));
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_secrets_come_from_stdin_and_never_from_arguments() {
    let bus = Bus::start().await;
    let add =
        ["provider", "add", "photos", "immich", "server=https://photos.example.org", "--allow-plain-http"];
    let (code, out, err) = bus.fed(&[&add[..], &["--secret-stdin"]].concat(), "s3cret\nignored\n");
    assert_eq!((code, out.as_str()), (0, format!("{}\n", UIDS[0]).as_str()), "{err}");
    let options = r#"{"allow_plain_http": "true", "server": "https://photos.example.org"}"#;
    assert!(bus.log.has(&format!("Add photos immich {options} [s3cret]")));
    let (code, _, err) = bus.fed(&add, "");
    assert!(code == 1 && err.contains("immich needs its api_key; use --no-secret"), "{err}");
    assert_eq!(bus.ctl(&[&add[..], &["--no-secret"]].concat()).0, 0);
    assert!(bus.log.has(&format!("Add photos immich {options} []")));
    let (code, _, err) = bus.fed(&[&add[..], &["api_key=s3cret"]].concat(), "s3cret\n");
    assert!(code == 2 && err.contains("api_key is a secret"), "{err}");
    assert!(!bus.log.has("Add photos immich {\"allow_plain_http\": \"true\", \"api_key\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn playlist_rows_far_apart_are_read_before_removal() {
    let bus = Bus::start().await;
    let (code, _, err) = bus.ctl(&["playlist", "remove-at", "Mine", "551", "3", "550"]);
    assert_eq!(code, 0, "{err}");
    assert!(bus.log.has("Get Mine 3 500") && bus.log.has("Get Mine 550 500") && !bus.log.has("Get Mine 551"));
    let expected = format!("{:?}", [UIDS[1], UIDS[3], UIDS[0]]);
    assert!(bus.log.has(&format!("RemoveEntries Mine [551, 3, 550] {expected}")));
    let (code, _, err) = bus.ctl(&["playlist", "remove-at", "Mine", "600"]);
    assert_eq!((code, err.as_str()), (1, "Error: the playlist has no entry 600\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_upgrade_tries_every_repository_before_failing_on_stale_ones() {
    let bus = Bus::start().await;
    let (code, out, _) = bus.ctl(&["pkg", "upgrade"]);
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(bus.log.has("Install 0123456789abcdef/org.example.dawn false false"));
    assert!(bus.log.has("Install fedcba9876543210/org.example.dusk false false"));
    let (code, out, _) = bus.ctl(&["pkg", "upgrade", "--allow-stale"]);
    assert_eq!((code, out.as_str()), (0, "5\n5\n"));
}
