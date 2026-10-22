use super::{Control, Grants, Hub, View, pace};
use crate::compositor::lock;
use crate::inputs1::{load, scene_id};
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::SeqCst};
use std::time::{Duration, Instant};
use wprs_core::model::{Input, SceneInputs};
use wprs_scene::PointerState;

const SEA: &str = "org.example.sea";

fn hub(wakes: Arc<AtomicU32>) -> Arc<Hub> {
    let wake = move || {
        wakes.fetch_add(1, SeqCst);
    };
    Arc::new(Hub::new(Grants::new(), Arc::new(wake), Arc::new(|| {})))
}

fn mode(path: &std::path::Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn permissions_are_stored_privately_and_can_be_revoked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data").join("inputs.json");
    let hub = hub(Arc::default());
    hub.set_grant(&path, SEA, Input::Audio, true).unwrap();
    hub.set_grant(&path, SEA, Input::Pointer, true).unwrap();
    assert_eq!(load(&path), Grants::from([(SEA.into(), [Input::Audio, Input::Pointer].into())]));
    assert_eq!((mode(&path), mode(path.parent().unwrap())), (0o600, 0o700));
    hub.set_grant(&path, SEA, Input::Audio, false).unwrap();
    assert_eq!(load(&path), Grants::from([(SEA.into(), [Input::Pointer].into())]));
    hub.set_grant(&path, SEA, Input::Pointer, false).unwrap();
    hub.set_grant(&path, SEA, Input::Pointer, false).unwrap();
    assert_eq!((load(&path), std::fs::read_to_string(&path).unwrap()), (Grants::new(), "{}".into()));
    std::fs::write(&path, "not json").unwrap();
    assert_eq!(load(&path), Grants::new());
}

#[test]
fn inputs_need_a_shown_active_scene_with_permission() {
    let view = View { active: true, covered: false, rect: None };
    let mut control = Control::default();
    let on = |c: &Control, input| !c.wants(input).is_empty();
    control.shown.insert("DP-1".into(), SceneInputs { scene: SEA.into(), audio: true, pointer: true });
    control.views.insert("DP-1".into(), view);
    assert!(!on(&control, Input::Audio) && !on(&control, Input::Pointer), "off by default");
    control.grants.insert("org.example.other".into(), [Input::Audio, Input::Pointer].into());
    assert!(!on(&control, Input::Audio), "granted to another scene");
    control.grants.insert(SEA.into(), [Input::Audio].into());
    assert!(on(&control, Input::Audio) && !on(&control, Input::Pointer));
    control.grants.insert(SEA.into(), [Input::Audio, Input::Pointer].into());
    control.views.insert("DP-1".into(), View { covered: true, ..view });
    assert!(on(&control, Input::Audio) && !on(&control, Input::Pointer), "covered screens get no pointer");
    control.views.insert("DP-1".into(), View { active: false, ..view });
    assert!(!on(&control, Input::Audio) && !on(&control, Input::Pointer), "paused screens get nothing");
    control.views.clear();
    assert!(!on(&control, Input::Audio), "an output that is not shown");
    control
        .shown
        .insert("DP-2".into(), SceneInputs { scene: "org.example.lake".into(), audio: true, pointer: true });
    assert!(control.allows("DP-1", Input::Audio) && control.allows("DP-1", Input::Pointer));
    assert!(
        !control.allows("DP-2", Input::Audio) && !control.allows("DP-3", Input::Audio),
        "only granted scenes hear"
    );
}

#[test]
fn only_outputs_whose_scene_declares_the_pointer_follow_it() {
    let view = View { active: true, covered: false, rect: Some([0.0, 0.0, 100.0, 100.0]) };
    let hub = hub(Arc::default());
    let mut control = lock(&hub.control);
    control.grants.insert(SEA.into(), [Input::Audio, Input::Pointer].into());
    control.shown.insert("DP-1".into(), SceneInputs { scene: SEA.into(), audio: true, pointer: false });
    control.shown.insert("DP-2".into(), SceneInputs { scene: SEA.into(), audio: false, pointer: true });
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
    let view = View { active: true, covered: false, rect: Some([100.0, 0.0, 200.0, 100.0]) };
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
