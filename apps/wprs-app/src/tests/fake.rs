use crate::args::{INVALID, fixture};
use crate::bus::Daemon;
use crate::calls::dispatch;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use wprs_client::{Client, parse};
use wprs_core::dbus::{BUS_NAME, OBJECT_PATH};
use wprs_core::model::{MODEL, State};
use zbus::connection::Builder;

struct FakeDaemon;

#[zbus::interface(name = "io.github.wallpaper_rs.Daemon1")]
impl FakeDaemon {
    #[zbus(property(emits_changed_signal = "const"))]
    fn model(&self) -> u32 {
        MODEL
    }

    fn get_state(&self) -> String {
        fixture("Daemon1", "GetState")
    }
}

struct FakeLibrary;

#[zbus::interface(name = "io.github.wallpaper_rs.Library1")]
impl FakeLibrary {
    fn add_source(&self, path: &str, recursive: bool) -> (String, u64) {
        (format!("{path} {recursive}"), 4)
    }

    fn set_tags(&self, uids: Vec<String>, add: Vec<String>, remove: Vec<String>) -> zbus::fdo::Result<()> {
        match uids.len() + add.len() + remove.len() {
            3 => Ok(()),
            _ => Err(zbus::fdo::Error::InvalidArgs("expected one of each".into())),
        }
    }

    fn request_thumbnails(&self, uids: Vec<String>, size: u32) -> HashMap<String, String> {
        uids.into_iter().map(|uid| (uid, format!("/tmp/{size}.jpg"))).collect()
    }
}

#[tokio::test]
async fn calls_reach_the_daemon_with_typed_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let listen = format!("<listen>unix:dir={}</listen><auth>EXTERNAL</auth>", dir.path().display());
    let policy = r#"<policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy>"#;
    std::fs::write(dir.path().join("bus.conf"), format!("<busconfig>{listen}{policy}</busconfig>")).unwrap();
    let mut bus = Command::new("dbus-daemon")
        .args(["--nofork", "--print-address", "--config-file"])
        .arg(dir.path().join("bus.conf"))
        .stdout(Stdio::piped())
        .spawn()
        .expect("dbus-daemon is installed");
    let mut address = String::new();
    BufReader::new(bus.stdout.take().unwrap()).read_line(&mut address).unwrap();
    let service = Builder::address(address.trim()).name(BUS_NAME);
    let service = service.serve_at(OBJECT_PATH, FakeDaemon).serve_at(OBJECT_PATH, FakeLibrary);
    let _service = service.build().await.unwrap();
    let client = Client::new(Builder::address(address.trim()).build().await.unwrap()).await.unwrap();
    let daemon = Daemon::Bus(client);
    let call =
        |iface, member, args: Value| dispatch(&daemon, iface, member, args.as_array().unwrap().clone());
    assert_eq!(call("Library1", "AddSource", json!(["/tmp/x", true])).await.unwrap(), r#"["/tmp/x true",4]"#);
    let thumbs = call("Library1", "RequestThumbnails", json!([["A"], 512])).await.unwrap();
    assert_eq!(thumbs, r#"[{"A":"/tmp/512.jpg"}]"#);
    assert_eq!(call("Library1", "SetTags", json!([["A"], ["sea"], ["old"]])).await.unwrap(), "[]");
    let state: State = parse(&call("Daemon1", "GetState", json!([])).await.unwrap()).unwrap();
    assert_eq!(state.outputs.len(), 2);
    let failed = call("Library1", "SetTags", json!([[], [], []])).await.unwrap_err();
    assert_eq!((failed.name.as_str(), failed.message.as_str()), (INVALID, "expected one of each"));
    let older = call("Playlists1", "List", json!([])).await.unwrap_err();
    assert_eq!(older.message, "wprsd is older than this client; install matching versions of wallpaper-rs");
    bus.kill().ok();
    bus.wait().ok();
}
