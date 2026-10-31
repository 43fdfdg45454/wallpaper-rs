use super::*;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use wprs_core::model::SceneInputs;
use zbus::OwnedVariant;

const PATH: &str = "/org/freedesktop/Notifications";
const NAME: &str = "org.freedesktop.Notifications";

pub(crate) struct Bus(Child, pub(crate) String);

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub(crate) fn private_bus() -> Option<Bus> {
    let args = ["--session", "--nofork", "--print-address=1"];
    let mut daemon = Command::new("dbus-daemon").args(args).stdout(Stdio::piped()).spawn().ok()?;
    let mut address = String::new();
    BufReader::new(daemon.stdout.take()?).read_line(&mut address).ok()?;
    Some(Bus(daemon, address.trim().to_owned()))
}

pub(crate) async fn connect(address: &str) -> zbus::Connection {
    zbus::connection::Builder::address(address).build().await.unwrap()
}

type Sent = (String, Option<String>);

struct Fake(tokio::sync::mpsc::UnboundedSender<Sent>);

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Fake {
    #[allow(clippy::too_many_arguments)]
    async fn notify(
        &self,
        _app: String,
        _replaces: u32,
        icon: String,
        _summary: String,
        _body: String,
        _actions: Vec<String>,
        hints: HashMap<String, OwnedVariant>,
        _timeout: i32,
    ) -> u32 {
        let entry = hints.get("desktop-entry").and_then(|v| String::try_from(v.clone()).ok());
        let _ = self.0.send((icon, entry));
        7
    }

    async fn close_notification(&self, _id: u32) {}
}

async fn notifications(address: &str) -> (zbus::Connection, tokio::sync::mpsc::UnboundedReceiver<Sent>) {
    let server = connect(address).await;
    server.request_name(NAME).await.unwrap();
    let (tx, asked) = tokio::sync::mpsc::unbounded_channel();
    server.object_server().at(PATH, Fake(tx)).await.unwrap();
    (server, asked)
}

async fn emit(
    from: &zbus::Connection,
    member: &str,
    args: &(u32, impl serde::Serialize + zbus::zvariant::Type),
) {
    from.emit_signal(None::<()>, PATH, NAME, member, args).await.unwrap();
}

#[tokio::test]
async fn only_the_notification_service_answers_for_the_user() {
    let Some(bus) = private_bus() else { return };
    let (server, mut asked) = notifications(&bus.1).await;
    for real in ["deny", "allow"] {
        let (client, spoofer) = (connect(&bus.1).await, connect(&bus.1).await);
        let asking = tokio::spawn(async move { ask(&client, ("A".into(), "B".into())).await.unwrap() });
        let (icon, entry) = asked.recv().await.unwrap();
        assert_eq!((icon.as_str(), entry.as_deref()), (APP, Some(APP)), "the question hides who asks");
        emit(&spoofer, "ActionInvoked", &(7, "allow")).await;
        emit(&server, "ActionInvoked", &(7, real)).await;
        let answer = asking.await.unwrap();
        assert_eq!(answer, Some(real == "allow"), "a signal from another program was taken as the answer");
    }
}

fn hub() -> Arc<Hub> {
    counted(Default::default())
}

fn counted(changes: Arc<std::sync::atomic::AtomicU32>) -> Arc<Hub> {
    let changed = move || {
        changes.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    };
    Arc::new(Hub::new(Default::default(), Arc::new(|| {}), Arc::new(changed)))
}

fn show(hub: &Hub, scene: &str, output: &str, key: &str) {
    let (scene, key, title, granted) = (scene.into(), key.into(), "Asked".into(), Default::default());
    let shown = SceneInputs {
        scene,
        audio: true,
        pointer: false,
        clicks: false,
        network: false,
        title,
        key,
        granted,
    };
    lock(&hub.control).shown.insert(output.into(), shown);
}

#[tokio::test]
async fn an_allowed_question_reaches_only_the_package_it_named() {
    let Some(bus) = private_bus() else { return };
    let ((server, mut asked), client) = (notifications(&bus.1).await, connect(&bus.1).await);
    let dir = tempfile::tempdir().unwrap();
    let (scene, hub, path) = ("org.example.named", hub(), dir.path().join("inputs.json"));
    let package = || (scene.to_owned(), "key:trusted".to_owned(), "Named".to_owned());
    for (twin, granted) in [(true, false), (false, true)] {
        show(&hub, scene, "DP-1", "key:trusted");
        let _ = grant(&client, hub.clone(), path.clone(), package(), Input::Audio).await;
        asked.recv().await.unwrap();
        if twin {
            show(&hub, scene, "DP-1", "sha256:twin");
        }
        emit(&server, "ActionInvoked", &(7, "allow")).await;
        let start = Instant::now();
        while lock(&ASKED).contains_key(&(scene.to_owned(), Input::Audio)) {
            assert!(start.elapsed() < Duration::from_secs(10), "the answer was not applied");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let control = lock(&hub.control);
        assert_eq!(control.granted(scene, "sha256:twin"), Default::default(), "the twin got the grant");
        assert_eq!(control.granted(scene, "key:trusted").contains(&Input::Audio), granted);
    }
}

#[tokio::test]
async fn a_dismissed_question_waits_and_a_decline_never_stops_asking() {
    let Some(bus) = private_bus() else { return };
    let ((server, mut asked), client) = (notifications(&bus.1).await, connect(&bus.1).await);
    let dir = tempfile::tempdir().unwrap();
    let changes = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let (scene, hub) = ("org.example.asked", counted(changes.clone()));
    let listed = |pick: fn((Grants, Grants)) -> Grants| pick(questions()).contains_key(scene);
    show(&hub, "org.example.asked", "DP-1", "sha256:aa");
    lock(&hub.control).declined.insert(scene.into(), [Input::Audio].into());
    let package = || (scene.to_owned(), "sha256:aa".to_owned(), "Asked".to_owned());
    let path = dir.path().join("inputs.json");
    let unconfirmed = |r: Result<()>| matches!(r, Err(Error::Named(name, _)) if name == UNCONFIRMED);
    assert!(unconfirmed(grant(&client, hub.clone(), path.clone(), package(), Input::Audio).await));
    asked.recv().await.expect("a declined input was not asked for again");
    assert!(listed(|q| q.0) && !listed(|q| q.1), "the open question is not reported");
    assert!(unconfirmed(grant(&client, hub.clone(), path.clone(), package(), Input::Audio).await));
    emit(&server, "NotificationClosed", &(7, 2u32)).await;
    let start = Instant::now();
    loop {
        let again = grant(&client, hub.clone(), path.clone(), package(), Input::Audio).await;
        if matches!(again, Err(Error::Fdo(fdo::Error::AccessDenied(_)))) {
            break;
        }
        assert!(
            unconfirmed(again) && start.elapsed() < Duration::from_secs(10),
            "no quiet time after closing"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(asked.try_recv().is_err(), "the user was asked again while the question was open or dismissed");
    assert!(lock(&hub.control).grants.is_empty());
    assert!(!listed(|q| q.0) && listed(|q| q.1), "the quiet time is not reported");
    assert!(changes.load(std::sync::atomic::Ordering::SeqCst) >= 2, "clients were not told");
}
