use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use wprs_scene::*;

fn runtime(timelines: Json, layers: Json) -> Runtime {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "timelines": timelines,
        "assets": { "a": { "path": "a.ogg" }, "b": { "path": "b.ogg" }, "c": { "path": "c.ogg" } },
        "layers": layers });
    let scene = Scene::from_json(&text.to_string()).unwrap();
    assert_eq!(scene.validate(&BTreeMap::new()), []);
    Runtime::new(Arc::new(scene), Values::new())
}

fn names(rt: &mut Runtime) -> Vec<String> {
    let fired = rt.take_fired().into_iter();
    fired.map(|f| format!("{}:{}", f.timeline, f.name.unwrap_or_else(|| "ended".into()))).collect()
}

fn box_x(rt: &Runtime) -> f32 {
    rt.frame([0.0, 0.0, 100.0, 50.0], Quality::Q3).draws[0].world.apply([0.0, 0.0])[0]
}

#[test]
fn events_fire_once_per_pass_even_when_frames_skip_them() {
    let events = json!([{ "t": 0, "name": "zero" }, { "t": 1, "name": "one" }, { "t": 1.5, "name": "late" }]);
    let tls = json!([{ "id": "r", "duration": 2, "loop": "repeat", "events": events },
                     { "id": "p", "duration": 2, "loop": "pingpong", "events": [{ "t": 1.5, "name": "bounce" }] },
                     { "id": "o", "duration": 1, "delay": 0.5, "events": [{ "t": 0.5, "name": "half" }] }]);
    let mut rt = runtime(tls, json!([]));
    rt.advance(0.05);
    assert_eq!(names(&mut rt), ["r:zero"]);
    rt.advance(1.2);
    assert_eq!(names(&mut rt), ["r:one", "o:half"]);
    rt.advance(0.3);
    assert_eq!(names(&mut rt), ["r:late", "p:bounce", "o:ended"]);
    rt.advance(1.0);
    assert_eq!(names(&mut rt), ["r:zero", "p:bounce"]);
    rt.advance(4.0);
    let skipped = names(&mut rt);
    assert_eq!(skipped.iter().filter(|n| *n == "r:one").count(), 2, "{skipped:?}");
    assert_eq!(skipped.iter().filter(|n| *n == "p:bounce").count(), 2, "{skipped:?}");
    rt.seek(0.6);
    assert_eq!(names(&mut rt), Vec::<String>::new(), "a seek replays quietly");
}

#[test]
fn events_start_timelines_and_wake_the_scene_only_when_heard() {
    let tls = json!([{ "id": "clock", "duration": 4, "loop": "repeat", "events": [{ "t": 3, "name": "go" }] },
                     { "id": "move", "duration": 1, "trigger": "event:go" }]);
    let layer = json!([{ "id": "box", "type": "solid", "color": "#ffffff", "size": [10, 10],
        "transform": { "position": { "timeline": "move", "keys": [[0, [0, 0]], [1, [50, 0]]] } } }]);
    let mut rt = runtime(tls, layer);
    rt.advance(0.5);
    let frame = rt.frame([0.0, 0.0, 100.0, 50.0], Quality::Q3);
    assert!(matches!(frame.wake, Wake::At(s) if (s - 2.5).abs() < 1e-3), "{:?}", frame.wake);
    rt.advance(3.0);
    assert!((box_x(&rt) - 25.0).abs() < 0.5, "{}", box_x(&rt));
    let quiet =
        runtime(json!([{ "id": "t", "duration": 4, "events": [{ "t": 3, "name": "go" }] }]), json!([]));
    assert_eq!(quiet.frame([0.0, 0.0, 100.0, 50.0], Quality::Q3).wake, Wake::Idle, "nothing listens");
}

#[test]
fn scripts_stop_speed_up_and_read_their_timelines() {
    let tls = json!([{ "id": "slide", "duration": 2, "loop": "repeat" }]);
    let layer = json!([{ "id": "box", "type": "solid", "color": "#ffffff", "size": [10, 10],
        "transform": { "position": { "timeline": "slide", "keys": [[0, [0, 0]], [2, [100, 0]]] } } }]);
    let mut rt = runtime(tls, layer);
    let send = |rt: &mut Runtime, c| {
        rt.set_script(ScriptOutputs { timelines: vec![("slide".into(), c)], ..Default::default() })
    };
    send(&mut rt, TimelineCommand::Speed(2.0));
    rt.advance(0.5);
    let course = &rt.playbacks()[0];
    assert_eq!(
        (course.id.as_str(), course.time, course.duration, course.playing, course.speed),
        ("slide", 1.0, 2.0, true, 2.0)
    );
    send(&mut rt, TimelineCommand::Stop);
    rt.advance(0.5);
    assert_eq!((rt.playbacks()[0].time, rt.playbacks()[0].playing, box_x(&rt)), (0.0, false, 0.0));
}

#[test]
fn wrapped_timelines_blend_their_last_key_into_the_first() {
    let tls = json!([{ "id": "w", "duration": 4, "loop": "repeat", "wrap": true },
                     { "id": "n", "duration": 4, "loop": "repeat" }]);
    let at = |id: &str, t: f64| {
        let layer = json!([{ "id": "box", "type": "solid", "color": "#ffffff", "size": [10, 10],
            "transform": { "position": { "timeline": id, "keys": [[1, [0, 0]], [3, [80, 0]]] } } }]);
        let mut rt = runtime(tls.clone(), layer);
        rt.advance(t);
        box_x(&rt)
    };
    assert_eq!(
        [at("w", 2.0), at("w", 3.5), at("w", 4.5), at("w", 5.0)].map(f32::round),
        [40.0, 60.0, 20.0, 0.0]
    );
    assert_eq!([at("n", 3.5), at("n", 4.5)], [80.0, 0.0]);
}

#[test]
fn sounds_tell_where_the_scene_says_they_are() {
    let tls =
        json!([{ "id": "t", "duration": 10, "loop": "repeat", "events": [{ "t": 4, "name": "ring" }] }]);
    let layers = json!([
        { "id": "always", "type": "sound", "sound": "a" },
        { "id": "keyed", "type": "sound", "sound": "a", "play": { "timeline": "t", "keys": [[0, false], [2, true], [6, false]] } },
        { "id": "rung", "type": "sound", "sounds": ["a", "b"], "loop": false, "play_on": "event:ring" }]);
    let mut rt = runtime(tls, layers);
    rt.set_lengths(BTreeMap::from([("a".into(), 1.0), ("b".into(), 2.0)]));
    rt.advance(3.0);
    let (voices, _) = rt.voices();
    let heard = voices.iter().map(|v| (v.play, (v.at * 1e3).round() / 1e3)).collect::<Vec<_>>();
    assert_eq!(heard, [(true, 3.0), (true, 1.0), (false, 0.0)]);
    rt.advance(1.5);
    let (voices, wake) = rt.voices();
    assert!(voices[2].play && (voices[2].at - 0.5).abs() < 1e-3, "{voices:?}");
    assert!(matches!(wake, Wake::At(s) if (s - 2.5).abs() < 1e-3) || wake == Wake::Frame, "{wake:?}");
    rt.advance(3.0);
    assert!(!rt.voices().0[2].play, "a list played once ends");
    rt.seek(13.0);
    let (voices, _) = rt.voices();
    assert!((voices[0].at - 13.0).abs() < 1e-3 && (voices[1].at - 1.0).abs() < 1e-3, "{voices:?}");
}

#[test]
fn playlists_follow_their_order_and_pauses() {
    let list = |order, pause: [f64; 2], repeat| Playlist {
        lengths: vec![1.0, 2.0, 3.0],
        order,
        pause,
        repeat,
        seed: 7,
    };
    let clip = |list: &Playlist, at: f64| match list.spot(at, &mut Walk::default()) {
        Spot::Clip { index, offset, .. } => Some((index, (offset * 10.0).round() / 10.0)),
        _ => None,
    };
    let sequence = list(Order::Sequence, [0.0; 2], true);
    assert_eq!(
        [0.5, 1.5, 4.0, 6.5].map(|t| clip(&sequence, t)),
        [Some((0, 0.5)), Some((1, 0.5)), Some((2, 1.0)), Some((0, 0.5))]
    );
    let once = list(Order::Sequence, [1.0, 1.0], false);
    assert_eq!(once.spot(1.5, &mut Walk::default()), Spot::Gap(0.5));
    assert_eq!((clip(&once, 2.5), once.spot(8.5, &mut Walk::default())), (Some((1, 0.5)), Spot::End));
    let random = list(Order::Random, [0.0, 2.0], true);
    let mut walk = Walk::default();
    let mut picks = Vec::new();
    let mut t = 0.0;
    while t < 200.0 {
        if let Spot::Clip { index, left, .. } = random.spot(t, &mut walk) {
            picks.push(index);
            t += left;
        }
        t += 0.01;
    }
    assert!(picks.windows(2).all(|w| w[0] != w[1]), "never the same clip twice in a row");
    assert!((0..3).all(|i| picks.contains(&i)));
    let again: Vec<_> = (0..20).map(|i| random.spot(f64::from(i) * 7.3, &mut Walk::default())).collect();
    let walked: Vec<_> = (0..20).map(|i| random.spot(f64::from(i) * 7.3, &mut walk)).collect();
    assert_eq!(again, walked, "the same seed gives the same list");
}

#[test]
fn still_scenes_with_events_wraps_and_sounds_draw_no_frames() {
    let tls = json!([{ "id": "intro", "duration": 1, "events": [{ "t": 0.5, "name": "half" }] },
                     { "id": "after", "duration": 0.5, "trigger": "event:half" },
                     { "id": "loop", "duration": 2, "loop": "repeat", "wrap": true, "autoplay": false }]);
    let layers = json!([
        { "id": "box", "type": "solid", "color": "#ffffff", "size": [10, 10],
          "opacity": { "timeline": "after", "keys": [[0, 0], [0.5, 1]] } },
        { "id": "still", "type": "solid", "color": "#ffffff", "size": [10, 10],
          "opacity": { "timeline": "loop", "keys": [[0, 0], [1, 1]] } },
        { "id": "bed", "type": "sound", "sound": "a" },
        { "id": "bell", "type": "sound", "sounds": ["b", "c"], "mode": "random", "loop": false, "play_on": "event:half" }]);
    let mut rt = runtime(tls, layers);
    rt.set_lengths(BTreeMap::from([("a".into(), 4.0), ("b".into(), 1.0), ("c".into(), 1.0)]));
    let view = [0.0, 0.0, 100.0, 50.0];
    assert!(matches!(rt.frame(view, Quality::Q3).wake, Wake::At(_)), "the intro waits for its event");
    rt.advance(0.7);
    assert_eq!(rt.frame(view, Quality::Q3).wake, Wake::Frame, "the triggered fade moves");
    assert!(matches!(rt.voices().1, Wake::At(s) if s > 0.0 && s < 1.0), "the bell wakes when it ends");
    rt.advance(2.0);
    assert_eq!(rt.frame(view, Quality::Q3).wake, Wake::Idle);
    let (voices, wake) = rt.voices();
    assert_eq!((voices[0].play, voices[1].play, wake), (true, false, Wake::Idle));
}

#[test]
fn timelines_started_by_an_event_fire_their_first_events_in_the_same_frame() {
    let b =
        json!([{ "t": 0, "name": "begin" }, { "t": 0.01, "name": "soon" }, { "t": 0.5, "name": "later" }]);
    let tls = json!([{ "id": "a", "duration": 4, "events": [{ "t": 1, "name": "go" }] },
                     { "id": "b", "duration": 1, "trigger": "event:go", "events": b },
                     { "id": "c", "duration": 1, "trigger": "event:begin", "events": [{ "t": 0, "name": "deep" }] }]);
    let mut rt = runtime(tls, json!([]));
    rt.advance(0.95);
    assert_eq!(names(&mut rt), Vec::<String>::new());
    rt.advance(0.1);
    assert_eq!(names(&mut rt), ["a:go", "b:begin", "b:soon", "c:deep"]);
    rt.advance(1.0);
    assert_eq!(names(&mut rt), ["b:later", "b:ended", "c:ended"]);
    let mut looped = runtime(
        json!([{ "id": "s", "duration": 1, "trigger": "event:s",
        "events": [{ "t": 0, "name": "s" }] }, { "id": "k", "duration": 1, "events": [{ "t": 0.5, "name": "s" }] }]),
        json!([]),
    );
    looped.advance(1.0);
    assert!(names(&mut looped).len() <= MAX_FIRED, "a timeline that restarts itself stops");
}

#[test]
fn a_negative_speed_runs_backwards_and_ends_at_the_start() {
    let mut rt = runtime(json!([{ "id": "o", "duration": 2 }]), json!([]));
    rt.advance(1.5);
    let back =
        ScriptOutputs { timelines: vec![("o".into(), TimelineCommand::Speed(-1.0))], ..Default::default() };
    rt.set_script(back);
    rt.advance(1.0);
    assert_eq!((rt.playbacks()[0].time, rt.playbacks()[0].playing), (0.5, true));
    rt.advance(1.0);
    assert_eq!(names(&mut rt), ["o:ended"]);
    assert_eq!((rt.playbacks()[0].time, rt.playbacks()[0].playing), (0.0, false));
    let tls = json!([{ "id": "r", "duration": 2, "loop": "repeat", "speed": -1, "delay": 5,
        "events": [{ "t": 1, "name": "half" }] }]);
    let mut rt = runtime(tls, json!([]));
    rt.advance(0.5);
    let time = |rt: &Runtime| (rt.playbacks()[0].time * 1e3).round() / 1e3;
    assert_eq!(time(&rt), 1.5, "a loop with a negative speed plays from its end at once");
    rt.advance(1.0);
    assert_eq!(names(&mut rt), ["r:half"]);
    assert_eq!(time(&rt), 0.5);
}

#[test]
fn only_what_the_platform_runs_listens_to_events() {
    let tls = json!([{ "id": "t", "duration": 4, "events": [{ "t": 1, "name": "tick" }, { "t": 3, "name": "go" }] },
                     { "id": "m", "duration": 1, "trigger": "event:go" }]);
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] }, "timelines": tls,
        "assets": { "a": { "path": "a.ogg" } }, "scripts": [{ "path": "s.js" }],
        "layers": [{ "id": "bell", "type": "sound", "sound": "a", "play_on": "event:tick" }] });
    let mut rt = Runtime::new(Arc::new(Scene::from_json(&text.to_string()).unwrap()), Values::new());
    let wake = |rt: &Runtime| rt.frame([0.0, 0.0, 100.0, 50.0], Quality::Q3).wake;
    assert_eq!(wake(&rt), Wake::At(1.0), "scripts and sounds hear every event");
    rt.set_listeners(true, false);
    assert_eq!(wake(&rt), Wake::At(1.0));
    rt.set_listeners(false, true);
    assert_eq!(wake(&rt), Wake::At(1.0), "the bell rings on tick");
    rt.set_listeners(false, false);
    assert_eq!(wake(&rt), Wake::At(3.0), "only the trigger listens");
}
