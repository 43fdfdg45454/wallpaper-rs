use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wprs_core::dbus::{BUS_NAME, INTERFACE, OBJECT_PATH};
use zbus::Connection;

mod filters;
mod replies;
mod services;
mod stores;
mod waits;

use services::{Daemon, Failure, Library};
use stores::{Inputs, Playlists, Profiles, Providers, Repos};

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
        let service = service
            .serve_at(OBJECT_PATH, Playlists(log.clone()))
            .serve_at(OBJECT_PATH, Profiles(log.clone()))
            .serve_at(OBJECT_PATH, Inputs(log.clone()));
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

async fn signal<B: serde::Serialize + zbus::zvariant::DynamicType>(bus: &Connection, name: &str, body: &B) {
    bus.emit_signal(None::<&str>, OBJECT_PATH, INTERFACE, name, body).await.unwrap();
}
