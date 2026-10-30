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

#[test]
fn inputs_need_a_shown_active_uncovered_scene_with_permission() {
    let view = View { active: true, covered: false, rect: None, clicks: false };
    let mut control = Control::default();
    let on = |c: &Control, input| !c.wants(input).is_empty();
    control.shown.insert("DP-1".into(), scene(SEA, "sha256:aa", true, true));
    control.views.insert("DP-1".into(), view);
    assert!(!on(&control, Input::Audio) && !on(&control, Input::Pointer), "off by default");
    let lake = Grants::from([("sha256:aa".into(), [Input::Audio, Input::Pointer].into())]);
    control.grants.insert("org.example.lake".into(), lake);
    assert!(!on(&control, Input::Audio), "granted to another scene");
    control.grants.extend(bound("sha256:aa", &[Input::Audio]));
    assert!(on(&control, Input::Audio) && !on(&control, Input::Pointer));
    control.grants.extend(bound("sha256:aa", &[Input::Audio, Input::Pointer]));
    control.views.insert("DP-1".into(), View { covered: true, ..view });
    assert!(!on(&control, Input::Audio) && !on(&control, Input::Pointer), "covered screens get nothing");
    control.views.insert("DP-1".into(), View { active: false, ..view });
    assert!(!on(&control, Input::Audio) && !on(&control, Input::Pointer), "paused screens get nothing");
    control.views.clear();
    assert!(!on(&control, Input::Audio), "an output that is not shown");
    control.shown.insert("DP-2".into(), scene("org.example.lake", "sha256:bb", true, true));
    assert!(control.allows("DP-1", Input::Audio) && control.allows("DP-1", Input::Pointer));
    assert!(
        !control.allows("DP-2", Input::Audio) && !control.allows("DP-3", Input::Audio),
        "only granted scenes hear"
    );
}

#[test]
fn only_outputs_whose_scene_declares_the_pointer_follow_it() {
    let view = View { active: true, covered: false, rect: Some([0.0, 0.0, 100.0, 100.0]), clicks: false };
    let hub = hub(Arc::default());
    let mut control = lock(&hub.control);
    control.grants.extend(bound("sha256:aa", &[Input::Audio, Input::Pointer]));
    control.shown.insert("DP-1".into(), scene(SEA, "sha256:aa", true, false));
    control.shown.insert("DP-2".into(), scene(SEA, "sha256:aa", false, true));
    control.views.extend(["DP-1", "DP-2", "DP-3"].map(|o| (o.to_string(), view)));
    assert_eq!(control.wants(Input::Pointer), ["DP-2".to_string()].into());
    control.followed = control.wants(Input::Pointer);
    drop(control);
    hub.set_pointer(50.0, 25.0);
    hub.store([("DP-3".into(), PointerState { pos: Some([0.5; 2]), ..PointerState::default() })]);
    let pointers = lock(&hub.pointers).clone();
    assert_eq!(pointers.keys().collect::<Vec<_>>(), ["DP-2"]);
    assert_eq!((pointers["DP-2"].pos, pointers["DP-2"].inside), (Some([0.5, 0.25]), true));
}

#[test]
fn positions_are_placed_on_the_screen_under_the_pointer() {
    let view = View { active: true, covered: false, rect: Some([100.0, 0.0, 200.0, 100.0]), clicks: false };
    assert_eq!(view.place(150.0, 50.0), Some([0.25, 0.5]));
    assert_eq!([view.place(99.0, 50.0), view.place(300.0, 50.0), view.place(150.0, -1.0)], [None; 3]);
    let hidden =
        [View { covered: true, ..view }, View { active: false, ..view }, View { rect: None, ..view }];
    assert!(hidden.iter().all(|v| v.place(150.0, 50.0).is_none()));
    assert!(scene_id(SEA).is_ok());
    assert!(["", "Org.sea", "a b", "a/b", &"a".repeat(256)].iter().all(|s| scene_id(s).is_err()));
}

#[tokio::test]
async fn pointer_moves_wake_the_renderer_at_most_sixty_times_a_second() {
    let wakes = Arc::new(AtomicU32::new(0));
    let hub = hub(wakes.clone());
    hub.set_pointer(1.0, 1.0);
    assert!(lock(&hub.pointers).is_empty(), "nothing is kept while the feed is off");
    lock(&hub.control).followed.insert("DP-1".into());
    tokio::spawn(pace(hub.clone()));
    let start = Instant::now();
    for i in 0..100 {
        hub.store([(
            "DP-1".into(),
            PointerState { pos: Some([i as f32 / 100.0, 0.5]), ..PointerState::default() },
        )]);
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    tokio::time::sleep(Duration::from_millis(40)).await;
    let (woken, most) = (wakes.load(SeqCst), start.elapsed().as_millis() as u32 / 16 + 2);
    assert!((2..=most).contains(&woken), "{woken} wakes for 100 moves, at most {most}");
    hub.store([("DP-1".into(), PointerState { pos: Some([0.5, 0.25]), ..PointerState::default() })]);
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert_eq!(wakes.load(SeqCst), woken + 1, "the last move always wakes");
}

#[test]
fn clicks_reach_a_scene_only_on_screens_that_take_them_and_with_the_grant() {
    let view = View { active: true, covered: false, rect: Some([0.0, 0.0, 100.0, 100.0]), clicks: false };
    let hub = hub(Arc::default());
    let mut control = lock(&hub.control);
    control.grants.extend(bound("sha256:aa", &[Input::Pointer]));
    control.shown.insert("DP-1".into(), SceneInputs { clicks: true, ..scene(SEA, "sha256:aa", false, true) });
    control.views.insert("DP-1".into(), view);
    assert!(control.allows("DP-1", Input::Pointer) && !control.allows("DP-1", Input::Clicks));
    control.grants.extend(bound("sha256:aa", &[Input::Pointer, Input::Clicks]));
    assert!(!control.allows("DP-1", Input::Clicks), "a screen that does not take clicks gave them");
    control.views.insert("DP-1".into(), View { clicks: true, ..view });
    assert_eq!(control.wants(Input::Clicks), ["DP-1".to_string()].into());
    control.followed = control.wants(Input::Clicks);
    drop(control);
    let at = |buttons| {
        ("DP-1".to_string(), PointerState { pos: Some([0.5; 2]), buttons, ..PointerState::default() })
    };
    hub.store([at(0), at(1), at(1), at(0)]);
    hub.set_pointer(20.0, 30.0);
    let (state, presses, allowed) = hub.touch("DP-1");
    let pressed = |b| Press { at: Some([0.5; 2]), buttons: b };
    assert_eq!((presses, allowed), (vec![pressed(1), pressed(0)], [true, true]));
    assert_eq!((state.pos, state.buttons), (Some([0.2, 0.3]), 0));
    hub.store((0..40).map(|i| at(i % 2)));
    assert_eq!(hub.touch("DP-1").1.len(), MAX_PRESSES, "presses were not bounded");
    hub.store([at(1)]);
    lock(&hub.control).views.insert("DP-1".into(), view);
    assert_eq!(hub.touch("DP-1"), (PointerState { buttons: 0, ..at(1).1 }, Vec::new(), [true, false]));
}
