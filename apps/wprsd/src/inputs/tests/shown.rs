use super::*;

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
    control.views.insert("DP-1".into(), view);
    control.blocked = true;
    assert!(!on(&control, Input::Audio) && on(&control, Input::Pointer), "the switch blocks only the sound");
    control.blocked = false;
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
    assert!(!hub.away("DP-2") && !hub.away("DP-3"), "a screen without reports refused clicks");
    hub.set_pointer(150.0, 25.0);
    assert!(hub.away("DP-2"), "clicks were taken on a screen the pointer is not on");
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
