use super::*;

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
