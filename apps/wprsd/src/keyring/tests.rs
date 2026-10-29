use super::*;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use zbus::object_server::SignalEmitter;

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
    next: u32,
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

fn unavailable_with(result: Result<()>) -> String {
    match result {
        Err(Error::Named(KEYRING_UNAVAILABLE, message)) => message,
        other => panic!("expected KeyringUnavailable, got {:?}", other.err()),
    }
}

async fn secret(keyring: &Keyring, id: &str, prompt: bool) -> Option<String> {
    match keyring.read(id, prompt).await.unwrap() {
        Stored::Secret(text) => Some(text.to_string()),
        Stored::Missing => None,
        Stored::Locked => Some("<locked>".into()),
    }
}

#[tokio::test]
async fn secrets_are_stored_read_and_deleted() {
    let bus = Bus::start();
    let keyring = Keyring::new(bus.connect().await);
    assert_eq!(unavailable_with(keyring.store("photos", "s3cret").await), ABSENT);
    assert!(!keyring.has("photos").await.unwrap() && secret(&keyring, "photos", true).await.is_none());
    keyring.delete("photos").await.unwrap();
    let vault = Shared::default();
    let _service = serve(&bus, &vault).await;
    for (id, value) in [("photos", "first"), ("photos", "s3cret"), ("album", "other")] {
        keyring.store(id, value).await.unwrap();
    }
    {
        let vault = vault.lock().unwrap();
        let labels: Vec<&str> = vault.items.iter().map(|item| item.3.as_str()).collect();
        assert_eq!(labels, ["wallpaper-rs: photos", "wallpaper-rs: album"]);
        assert_eq!(vault.items[0].1["xdg:schema"], SCHEMA);
    }
    assert!(keyring.has("photos").await.unwrap());
    assert_eq!(secret(&keyring, "photos", false).await.as_deref(), Some("s3cret"));
    keyring.delete("photos").await.unwrap();
    assert!(!keyring.has("photos").await.unwrap() && keyring.has("album").await.unwrap());
}

#[tokio::test]
async fn locked_keyrings_prompt_only_when_allowed() {
    let bus = Bus::start();
    let keyring = Keyring::new(bus.connect().await);
    let vault = Shared::new(Mutex::new(Vault { locked: true, ..Vault::default() }));
    let _service = serve(&bus, &vault).await;
    let refused = unavailable_with(keyring.store("photos", "s3cret").await);
    assert_eq!(refused, "unlock the keyring and try again");
    assert!(vault.lock().unwrap().items.is_empty());
    vault.lock().unwrap().accept = true;
    keyring.store("photos", "s3cret").await.unwrap();
    vault.lock().unwrap().locked = true;
    assert!(keyring.has("photos").await.unwrap());
    assert_eq!(secret(&keyring, "photos", false).await.as_deref(), Some("<locked>"));
    assert_eq!(secret(&keyring, "photos", true).await.as_deref(), Some("s3cret"));
    vault.lock().unwrap().locked = true;
    keyring.delete("photos").await.unwrap();
    assert!(vault.lock().unwrap().items.is_empty());
}

#[tokio::test]
async fn orphan_secrets_go_only_without_a_prompt() {
    let bus = Bus::start();
    let keyring = Keyring::new(bus.connect().await);
    assert!(keyring.discard("photos").await.unwrap());
    let vault = Shared::default();
    let _service = serve(&bus, &vault).await;
    keyring.store("photos", "old").await.unwrap();
    vault.lock().unwrap().locked = true;
    assert!(!keyring.discard("photos").await.unwrap());
    assert_eq!(vault.lock().unwrap().items.len(), 1);
    vault.lock().unwrap().locked = false;
    assert!(keyring.discard("photos").await.unwrap());
    assert!(vault.lock().unwrap().items.is_empty());
}

#[tokio::test]
async fn a_keyring_that_does_not_answer_times_out() {
    let bus = Bus::start();
    let vault = Shared::new(Mutex::new(Vault { hang: true, ..Vault::default() }));
    let _service = serve(&bus, &vault).await;
    let (lookup, write) = (Duration::from_millis(200), Duration::from_millis(300));
    let keyring = Keyring { lookup, write, ..Keyring::new(bus.connect().await) };
    let started = std::time::Instant::now();
    assert_eq!(unavailable_with(keyring.has("photos").await.map(drop)), SLOW);
    assert_eq!(unavailable_with(keyring.read("photos", false).await.map(drop)), SLOW);
    assert_eq!(unavailable_with(keyring.store("photos", "s3cret").await), SLOW);
    assert_eq!(unavailable_with(keyring.delete("photos").await), SLOW);
    assert_eq!(unavailable_with(keyring.discard("photos").await.map(drop)), SLOW);
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn after_a_lookup_times_out_the_next_ones_fail_at_once_and_answers_are_remembered() {
    let bus = Bus::start();
    let vault = Shared::default();
    let _service = serve(&bus, &vault).await;
    let lookup = Duration::from_millis(300);
    let keyring = Keyring { lookup, ..Keyring::new(bus.connect().await) };
    keyring.store("photos", "s3cret").await.unwrap();
    assert_eq!((keyring.known("photos"), keyring.known("album")), (Some(true), None));
    vault.lock().unwrap().hang = true;
    assert_eq!(unavailable_with(keyring.has("album").await.map(drop)), SLOW);
    let started = std::time::Instant::now();
    assert_eq!(unavailable_with(keyring.has("photos").await.map(drop)), SLOW);
    assert_eq!(unavailable_with(keyring.read("photos", false).await.map(drop)), SLOW);
    assert_eq!(unavailable_with(keyring.properties().has("photos").await.map(drop)), SLOW);
    assert!(started.elapsed() < Duration::from_millis(100));
    vault.lock().unwrap().hang = false;
    *keyring.silent.lock().unwrap() = None;
    assert!(!keyring.has("album").await.unwrap() && keyring.known("album") == Some(false));
    keyring.delete("photos").await.unwrap();
    assert_eq!(keyring.known("photos"), Some(false));
}
