use super::super::*;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use zbus::object_server::SignalEmitter;
use zbus::zvariant::OwnedVariant;

const PROMPT: &str = "/org/freedesktop/secrets/prompt/1";

pub struct Bus {
    daemon: Child,
    pub address: String,
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

    pub async fn connect(&self) -> zbus::Connection {
        zbus::connection::Builder::address(self.address.as_str()).build().await.unwrap()
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        self.daemon.kill().ok();
        self.daemon.wait().ok();
    }
}

type Entry = (String, HashMap<String, String>, Vec<u8>, String);

#[derive(Default)]
pub struct Vault {
    pub items: Vec<Entry>,
    pub locked: bool,
    pub accept: bool,
    pub hang: bool,
    pub next: u32,
}

pub type Shared = Arc<Mutex<Vault>>;

fn path(text: &str) -> OwnedObjectPath {
    OwnedObjectPath::try_from(text).unwrap()
}

struct MockService(Shared);

impl MockService {
    async fn stall(&self) {
        let hang = self.0.lock().unwrap().hang;
        if hang {
            std::future::pending::<()>().await;
        }
    }
}

#[zbus::interface(name = "org.freedesktop.Secret.Service")]
impl MockService {
    async fn open_session(
        &self,
        algorithm: &str,
        _input: OwnedVariant,
    ) -> zbus::fdo::Result<(OwnedVariant, OwnedObjectPath)> {
        self.stall().await;
        let plain = algorithm == "plain";
        let output = Variant::from("").try_to_owned().unwrap();
        plain
            .then(|| (output, path("/org/freedesktop/secrets/session/1")))
            .ok_or_else(|| zbus::fdo::Error::NotSupported(algorithm.into()))
    }

    async fn search_items(&self, attributes: HashMap<String, String>) -> (Paths, Paths) {
        self.stall().await;
        let vault = self.0.lock().unwrap();
        let matches = |item: &&Entry| attributes.iter().all(|(key, value)| item.1.get(key) == Some(value));
        let found: Paths = vault.items.iter().filter(matches).map(|item| path(&item.0)).collect();
        if vault.locked { (Vec::new(), found) } else { (found, Vec::new()) }
    }

    fn unlock(&self, objects: Paths) -> (Paths, OwnedObjectPath) {
        if !self.0.lock().unwrap().locked {
            return (objects, path("/"));
        }
        (Vec::new(), path(PROMPT))
    }

    fn get_secrets(&self, items: Paths, session: OwnedObjectPath) -> HashMap<OwnedObjectPath, Secret> {
        let vault = self.0.lock().unwrap();
        let stored = vault.items.iter().filter(|item| items.iter().any(|p| p.as_str() == item.0));
        let secret = |item: &Entry| (session.clone(), Vec::new(), item.2.clone(), "text/plain".into());
        stored.map(|item| (path(&item.0), secret(item))).collect()
    }
}

struct MockCollection(Shared);

#[zbus::interface(name = "org.freedesktop.Secret.Collection")]
impl MockCollection {
    fn create_item(
        &self,
        properties: HashMap<String, OwnedVariant>,
        secret: Secret,
        replace: bool,
    ) -> (OwnedObjectPath, OwnedObjectPath) {
        let text = |key: &str| properties[key].try_clone().unwrap();
        let attributes: HashMap<String, String> =
            text("org.freedesktop.Secret.Item.Attributes").try_into().unwrap();
        let label: String = text("org.freedesktop.Secret.Item.Label").try_into().unwrap();
        let item = {
            let mut vault = self.0.lock().unwrap();
            vault.items.retain(|item| !replace || item.1 != attributes);
            vault.next += 1;
            let item = format!("{}/{}", COLLECTIONS[1], vault.next);
            vault.items.push((item.clone(), attributes, secret.2, label));
            item
        };
        (path(&item), path("/"))
    }

    #[zbus(property)]
    fn locked(&self) -> bool {
        self.0.lock().unwrap().locked
    }
}

struct MockItem(Shared, String);

#[zbus::interface(name = "org.freedesktop.Secret.Item")]
impl MockItem {
    fn delete(&self) -> OwnedObjectPath {
        self.0.lock().unwrap().items.retain(|item| item.0 != self.1);
        path("/")
    }
}

struct MockPrompt(Shared);

#[zbus::interface(name = "org.freedesktop.Secret.Prompt")]
impl MockPrompt {
    async fn prompt(&self, _window_id: &str, #[zbus(signal_emitter)] emitter: SignalEmitter<'_>) {
        let dismissed = {
            let mut vault = self.0.lock().unwrap();
            vault.locked &= !vault.accept;
            !vault.accept
        };
        Self::completed(&emitter, dismissed, Variant::from("")).await.unwrap();
    }

    fn dismiss(&self) {}

    #[zbus(signal)]
    async fn completed(emitter: &SignalEmitter<'_>, dismissed: bool, result: Variant<'_>)
    -> zbus::Result<()>;
}

pub async fn serve(bus: &Bus, vault: &Shared) -> zbus::Connection {
    let builder = zbus::connection::Builder::address(bus.address.as_str()).name(SERVICE);
    let builder = builder.serve_at("/org/freedesktop/secrets", MockService(vault.clone()));
    let builder = builder.serve_at(PROMPT, MockPrompt(vault.clone()));
    let items = (1..=8).map(|n| format!("{}/{n}", COLLECTIONS[1]));
    let builder = items.fold(builder, |b, item| b.serve_at(item.clone(), MockItem(vault.clone(), item)));
    builder.serve_at(COLLECTIONS[1], MockCollection(vault.clone())).build().await.unwrap()
}
