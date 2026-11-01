use serde_json::json;
use std::sync::Arc;
use wprs_scene::*;

const VIEW: [f32; 4] = [0.0, 0.0, 100.0, 50.0];

fn runtime(extra: serde_json::Value) -> Runtime {
    let mut doc = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "layers": [
        { "id": "back", "type": "solid", "color": "#000000" },
        { "id": "a", "type": "solid", "color": "#ffffff", "size": [20, 10], "solid": true }] });
    for (k, v) in extra.as_object().into_iter().flatten() {
        doc[k] = v.clone();
    }
    Runtime::new(Arc::new(Scene::from_json(&doc.to_string()).unwrap()), Values::new())
}

fn hitbox(layer: &str, [x, y, w, h]: [f32; 4]) -> Hitbox {
    let to_box = Affine::scale([1.0 / w, 1.0 / h]).then(Affine::translate([-x, -y]));
    let (to_local, cell) = (Affine::translate([-x, -y]), [0.0, 0.0, 1.0, 1.0]);
    Hitbox {
        layer: layer.into(),
        to_box,
        to_local,
        cell,
        alpha: 1.0,
        threshold: None,
        coverage: None,
        unlean: None,
    }
}

fn boxes() -> Hitboxes {
    Hitboxes(vec![hitbox("a", [10.0, 10.0, 20.0, 10.0]), hitbox("b", [20.0, 10.0, 20.0, 10.0])])
}

fn at(x: f32, y: f32, buttons: u8) -> Press {
    Press { at: Some([x / 100.0, y / 50.0]), buttons }
}

fn point(rt: &mut Runtime, presses: &[Press], allowed: (bool, bool)) -> bool {
    if let Some(p) = presses.last() {
        let inside = p.at.is_some();
        rt.set_pointer(PointerState { pos: p.at, buttons: p.buttons, inside, ..PointerState::default() });
    }
    rt.point(&boxes(), VIEW, presses, allowed)
}

fn events(rt: &mut Runtime, presses: &[Press], allowed: (bool, bool)) -> Vec<String> {
    point(rt, presses, allowed);
    rt.take_touches().iter().map(|t| format!("{:?} {} {:.1?}", t.kind, t.layer, t.local)).collect()
}

#[test]
fn the_topmost_layer_under_the_pointer_gets_enter_move_down_up_and_click() {
    let mut rt = runtime(json!({ "scripts": [{ "path": "a.js" }] }));
    assert_eq!(boxes().hit([25.0, 15.0]).map(|h| h.0), Some("b"));
    assert_eq!(boxes().hit([15.0, 15.0]), Some(("a", [5.0, 5.0])));
    assert_eq!(boxes().hit([5.0, 5.0]), None);
    let seen = events(&mut rt, &[at(15.0, 15.0, 0), at(15.0, 15.0, 1), at(16.0, 15.0, 0)], (true, true));
    let want = [
        "Enter a [5.0, 5.0]",
        "Move a [5.0, 5.0]",
        "Down a [5.0, 5.0]",
        "Move a [6.0, 5.0]",
        "Up a [6.0, 5.0]",
        "Click a [6.0, 5.0]",
    ];
    assert_eq!(seen, want);
    let seen = events(&mut rt, &[at(35.0, 15.0, 0), at(50.0, 40.0, 0)], (true, true));
    assert_eq!(
        seen,
        ["Leave a [6.0, 5.0]", "Enter b [15.0, 5.0]", "Move b [15.0, 5.0]", "Leave b [15.0, 5.0]"]
    );
}

#[test]
fn a_pressed_layer_keeps_the_pointer_until_release_and_clicks_need_both_ends_on_it() {
    let mut rt = runtime(json!({ "scripts": [{ "path": "a.js" }] }));
    let drag = [at(15.0, 15.0, 1), at(60.0, 40.0, 1), at(80.0, 45.0, 0)];
    let seen = events(&mut rt, &drag, (true, true));
    let want = [
        "Enter a [5.0, 5.0]",
        "Move a [5.0, 5.0]",
        "Down a [5.0, 5.0]",
        "Leave a [5.0, 5.0]",
        "Move a [50.0, 30.0]",
        "Move a [70.0, 35.0]",
        "Up a [70.0, 35.0]",
    ];
    assert_eq!(seen, want);
    let seen = events(&mut rt, &[at(15.0, 15.0, 0), at(15.0, 15.0, 1), at(15.0, 15.0, 0)], (false, true));
    assert_eq!(
        seen,
        ["Down a [5.0, 5.0]", "Up a [5.0, 5.0]", "Click a [5.0, 5.0]"],
        "clicks without the pointer"
    );
    let seen = events(&mut rt, &[at(15.0, 15.0, 1), at(15.0, 15.0, 0)], (true, false));
    assert_eq!(seen, ["Enter a [5.0, 5.0]"], "the pointer without clicks gave presses");
}

#[test]
fn clicks_enter_and_leave_start_their_timelines_without_scripts() {
    let timelines = json!([
        { "id": "on", "duration": 1, "autoplay": false, "trigger": "click:a" },
        { "id": "in", "duration": 1, "autoplay": false, "trigger": "enter:a" },
        { "id": "out", "duration": 1, "autoplay": false, "trigger": "leave:a" }]);
    let mut rt = runtime(json!({ "timelines": timelines }));
    let playing =
        |rt: &Runtime| rt.playbacks().iter().filter(|c| c.playing).map(|c| c.id.clone()).collect::<Vec<_>>();
    assert!(point(&mut rt, &[at(15.0, 15.0, 0)], (true, true)));
    assert_eq!(playing(&rt), ["in"]);
    assert!(rt.take_touches().is_empty(), "touches were kept for a scene without scripts");
    point(&mut rt, &[at(15.0, 15.0, 1), at(15.0, 15.0, 0), at(5.0, 5.0, 0)], (true, true));
    assert_eq!(playing(&rt), ["on", "in", "out"]);
    assert!(!rt.point(&boxes(), VIEW, &[], (true, true)), "a still pointer made events");
}

#[test]
fn alpha_thresholds_test_the_coverage_of_the_layer() {
    let coverage = Arc::new(AlphaMap { size: [2, 1], alpha: vec![0, 255] });
    let solid =
        Hitbox { threshold: Some(0.5), coverage: Some(coverage), ..hitbox("a", [0.0, 0.0, 20.0, 10.0]) };
    assert!(!solid.holds([5.0, 5.0]) && solid.holds([15.0, 5.0]));
    assert!(!Hitbox { alpha: 0.25, ..solid.clone() }.holds([15.0, 5.0]), "the opacity of the layer counts");
    assert!(Hitbox { threshold: None, ..solid }.holds([5.0, 5.0]), "a plain solid layer takes its whole box");
}

#[test]
fn scripts_play_pause_stop_and_turn_down_sound_layers() {
    let sound = json!({ "id": "bell", "type": "sound", "sound": "bell", "play_on": "beat", "loop": false });
    let mut rt = runtime(json!({ "assets": { "bell": { "path": "bell.ogg" } }, "layers": [sound] }));
    rt.set_lengths([("bell".to_string(), 10.0)].into());
    let control = |commands: &[SoundCommand], volume: Option<f32>| {
        let sounds = [("bell".to_string(), SoundControl { commands: commands.to_vec(), volume })].into();
        ScriptOutputs { sounds, ..ScriptOutputs::default() }
    };
    let voice = |rt: &Runtime| rt.voices().0[0];
    assert!(!voice(&rt).play && rt.sounds_playing().is_empty());
    rt.advance(1.0);
    rt.set_script(control(&[SoundCommand::Play], Some(0.5)));
    rt.advance(2.0);
    let v = voice(&rt);
    assert!(v.play && (v.at - 2.0).abs() < 1e-3 && (v.volume - 0.5).abs() < 1e-6, "{v:?}");
    assert_eq!(rt.sounds_playing(), ["bell"]);
    rt.set_script(control(&[SoundCommand::Pause], None));
    rt.advance(3.0);
    assert!(!voice(&rt).play);
    rt.set_script(control(&[SoundCommand::Play], None));
    rt.advance(1.0);
    assert!((voice(&rt).at - 3.0).abs() < 1e-3, "a paused sound did not go on from its place");
    rt.set_script(control(&[SoundCommand::Stop, SoundCommand::Play], Some(7.0)));
    let v = voice(&rt);
    assert!(v.play && v.at < 1e-3 && v.volume <= 1.0, "{v:?}");
    rt.advance(11.0);
    assert!(!voice(&rt).play && rt.sounds_playing().is_empty(), "a sound that plays once kept playing");
}

#[test]
fn clicks_without_the_pointer_span_frames() {
    let timelines = json!([{ "id": "on", "duration": 1, "autoplay": false, "trigger": "click:a" }]);
    let mut rt = runtime(json!({ "scripts": [{ "path": "a.js" }], "timelines": timelines }));
    rt.set_pointer(PointerState::default());
    let mut frame = |presses: &[Press]| {
        rt.point(&boxes(), VIEW, presses, (false, true));
        rt.take_touches().iter().map(|t| format!("{:?} {}", t.kind, t.layer)).collect::<Vec<_>>()
    };
    assert_eq!(frame(&[at(15.0, 15.0, 1)]), ["Down a"]);
    assert_eq!(frame(&[]), Vec::<String>::new(), "a frame without presses released the button");
    assert_eq!(frame(&[at(15.0, 15.0, 0)]), ["Up a", "Click a"]);
    assert!(rt.playbacks().iter().any(|c| c.id == "on" && c.playing));
}
