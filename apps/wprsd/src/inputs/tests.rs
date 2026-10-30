use super::{Bound, Control, Grants, Hub, View, pace};
use crate::compositor::lock;
use crate::inputs::{load, load_bound};
use crate::inputs1::scene_id;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::SeqCst};
use std::time::{Duration, Instant};
use wprs_core::model::{Input, SceneInputs};
use wprs_scene::{MAX_PRESSES, PointerState, Press};

mod shown;

const SEA: &str = "org.example.sea";

fn hub(wakes: Arc<AtomicU32>) -> Arc<Hub> {
    let wake = move || {
        wakes.fetch_add(1, SeqCst);
    };
    Arc::new(Hub::new(Default::default(), Arc::new(wake), Arc::new(|| {})))
}

fn mode(path: &std::path::Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn scene(scene: &str, key: &str, audio: bool, pointer: bool) -> SceneInputs {
    let (scene, key, title, granted) = (scene.into(), key.into(), String::new(), Default::default());
    SceneInputs { scene, audio, pointer, clicks: false, title, key, granted }
}

fn bound(key: &str, inputs: &[Input]) -> Bound {
    Bound::from([(SEA.into(), Grants::from([(key.into(), inputs.iter().copied().collect())]))])
}

fn showing(hub: &Hub, output: &str, key: &str) {
    lock(&hub.control).shown.insert(output.into(), scene(SEA, key, true, true));
}

#[test]
fn permissions_are_stored_privately_and_can_be_revoked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data").join("inputs.json");
    let hub = hub(Arc::default());
    assert!(
        hub.grant(&path, SEA, "sha256:aa", Input::Audio).is_err(),
        "granted to a scene that is not shown"
    );
    showing(&hub, "DP-1", "sha256:aa");
    hub.grant(&path, SEA, "sha256:aa", Input::Audio).unwrap();
    hub.grant(&path, SEA, "sha256:aa", Input::Pointer).unwrap();
    assert_eq!(load_bound(&path), bound("sha256:aa", &[Input::Audio, Input::Pointer]));
    assert_eq!((mode(&path), mode(path.parent().unwrap())), (0o600, 0o700));
    hub.revoke(&path, SEA, Input::Audio).unwrap();
    assert_eq!(load_bound(&path), bound("sha256:aa", &[Input::Pointer]));
    hub.revoke(&path, SEA, Input::Pointer).unwrap();
    hub.revoke(&path, SEA, Input::Pointer).unwrap();
    assert_eq!((load_bound(&path), std::fs::read_to_string(&path).unwrap()), (Bound::new(), "{}".into()));
    std::fs::write(&path, "not json").unwrap();
    assert_eq!(load_bound(&path), Bound::new());
}

#[test]
fn a_package_with_the_same_id_does_not_inherit_its_grants() {
    let dir = tempfile::tempdir().unwrap();
    let (path, hub) = (dir.path().join("inputs.json"), hub(Arc::default()));
    showing(&hub, "DP-1", "key:signer");
    assert_eq!(lock(&hub.control).package(SEA, None).unwrap().0, "key:signer");
    hub.grant(&path, SEA, "key:signer", Input::Audio).unwrap();
    showing(&hub, "DP-2", "sha256:other");
    assert!(lock(&hub.control).package(SEA, None).is_err(), "a bare id chose one of two packages");
    assert!(hub.grant(&path, SEA, "sha256:gone", Input::Pointer).is_err(), "granted to a package not shown");
    hub.grant(&path, SEA, "key:signer", Input::Pointer).unwrap();
    let control = lock(&hub.control);
    assert!(control.allows("DP-1", Input::Audio) && !control.allows("DP-2", Input::Audio));
    assert_eq!(control.granted(SEA, "key:signer"), [Input::Audio, Input::Pointer].into());
    assert_eq!(control.granted(SEA, "sha256:other"), Default::default());
    drop(control);
    std::fs::write(
        &path,
        format!(r#"{{"{SEA}": ["audio"], "org.example.lake": {{"sha256:aa": ["pointer"]}}}}"#),
    )
    .unwrap();
    let kept = Bound::from([(
        "org.example.lake".into(),
        Grants::from([("sha256:aa".into(), [Input::Pointer].into())]),
    )]);
    assert_eq!(load_bound(&path), kept, "grants of an older version are asked for again");
}

#[test]
fn declining_is_remembered_until_the_input_is_granted() {
    let dir = tempfile::tempdir().unwrap();
    let (path, hub) = (dir.path().join("inputs.json"), hub(Arc::default()));
    let declined = crate::inputs::declined_path(&path);
    hub.revoke(&path, SEA, Input::Audio).unwrap();
    assert_eq!(load(&declined), Grants::from([(SEA.into(), [Input::Audio].into())]));
    assert_eq!((lock(&hub.control).declined.clone(), mode(&declined)), (load(&declined), 0o600));
    showing(&hub, "DP-1", "sha256:aa");
    hub.grant(&path, SEA, "sha256:aa", Input::Audio).unwrap();
    assert_eq!(load(&declined), Grants::new());
    assert_eq!(load_bound(&path), bound("sha256:aa", &[Input::Audio]));
}

#[test]
fn a_declined_question_spares_the_twins_and_old_clients_see_the_shown_package() {
    let dir = tempfile::tempdir().unwrap();
    let (path, hub) = (dir.path().join("inputs.json"), hub(Arc::default()));
    let reported = || crate::inputs1::reported(&lock(&hub.control));
    showing(&hub, "DP-1", "key:signer");
    hub.grant(&path, SEA, "key:signer", Input::Audio).unwrap();
    showing(&hub, "DP-1", "sha256:twin");
    assert_eq!(reported(), Grants::new(), "a twin was reported as allowed");
    hub.decline(&path, SEA, "sha256:twin", Input::Audio).unwrap();
    assert_eq!(load_bound(&path), bound("key:signer", &[Input::Audio]), "the decline revoked a twin");
    assert_eq!(lock(&hub.control).declined, Grants::from([(SEA.into(), [Input::Audio].into())]));
    showing(&hub, "DP-1", "key:signer");
    assert_eq!(reported(), Grants::from([(SEA.into(), [Input::Audio].into())]));
    lock(&hub.control).shown.clear();
    assert_eq!(reported(), Grants::from([(SEA.into(), [Input::Audio].into())]), "a grant was hidden");
}
