use serde_json::json;
use std::sync::Arc;
use wprs_scene::*;

fn runtime(autoplay: bool) -> Runtime {
    let text = json!({ "schema": SCHEMA, "canvas": { "size": [100, 50] },
        "timelines": [{ "id": "t", "duration": 4, "loop": "repeat", "autoplay": autoplay }],
        "assets": { "a": { "path": "a.ogg" } },
        "layers": [{ "id": "s", "type": "sound", "sound": "a",
                     "play": { "timeline": "t", "keys": [[0, false], [1, true]] } }] });
    let scene = Scene::from_json(&text.to_string()).unwrap();
    Runtime::new(Arc::new(scene), Values::new())
}

fn timeline(command: TimelineCommand) -> ScriptOutputs {
    ScriptOutputs { timelines: vec![("t".into(), command)], ..ScriptOutputs::default() }
}

fn heard(rt: &mut Runtime, step: f64) -> (bool, f64) {
    rt.advance(step);
    let voice = rt.voices().0[0];
    (voice.play, (voice.at * 1e3).round() / 1e3)
}

#[test]
fn sounds_keep_the_place_of_their_timeline_through_pauses_and_loops() {
    let mut rt = runtime(true);
    assert_eq!(heard(&mut rt, 2.5), (true, 1.5));
    rt.set_script(timeline(TimelineCommand::Pause));
    assert_eq!(heard(&mut rt, 0.5), (true, 2.0), "a stopped timeline lets its sound run on");
    assert_eq!(heard(&mut rt, 0.5), (true, 2.5));
    rt.set_script(timeline(TimelineCommand::Play));
    assert_eq!(heard(&mut rt, 0.25), (true, 1.75), "playing again goes back to where the timeline is");
    assert_eq!(heard(&mut rt, 1.0), (true, 2.75));
    assert_eq!(heard(&mut rt, 0.5), (false, 0.0), "the loop closes the sound");
    assert_eq!(heard(&mut rt, 1.0), (true, 0.25), "and the next turn starts it from its beginning");
    rt.set_script(timeline(TimelineCommand::Seek(3.0)));
    assert_eq!(heard(&mut rt, 0.0), (true, 2.0), "a seek moves the sound with the timeline");
}

#[test]
fn a_timeline_started_later_plays_its_sound_from_the_timeline_position() {
    let mut rt = runtime(false);
    assert_eq!(heard(&mut rt, 2.0), (false, 0.0));
    rt.set_script(timeline(TimelineCommand::Seek(2.5)));
    rt.set_script(timeline(TimelineCommand::Play));
    assert_eq!(heard(&mut rt, 0.5), (true, 2.0), "not from zero");
}
