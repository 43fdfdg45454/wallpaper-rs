use super::ask;
use crate::stage::Stage;
use crate::{pack, playing};
use serde_json::{Value, json};
use std::path::Path;

const TRACK: &str = r#"{"title": "Night Drive", "artist": "Example Artist", "playing": true, "position": 5, "duration": 200}"#;

fn shown(stage: &mut Stage) -> (String, u64) {
    stage.hear_media();
    let text = stage.runtime().read("title").and_then(|l| l.text).unwrap_or_default();
    (text, stage.runtime().media().0.cover)
}

pub(super) fn pulse_shows_what_plays(pulse: &str, path: &str) {
    let form = ask(json!({ "op": "scene", "uid": pulse, "languages": [] }));
    assert_eq!((&form["uses"]["media"], &form["granted"]["media"]), (&json!(true), &json!(false)), "{form}");
    let mut stage = Stage::new(pulse.into(), pack::load(Path::new(path)).unwrap());
    playing::allow(true);
    playing::heard(Some(TRACK), Some(&[0xffff_0000_u32 as i32; 16]), 4);
    assert!(!stage.hears_media());
    let waiting = || ("Nothing is playing".to_owned(), 0);
    assert_eq!(shown(&mut stage), waiting(), "only the placeholder without the grant");
    assert_eq!(ask(json!({ "op": "grant", "uid": pulse, "input": "media", "on": true })), Value::Null);
    stage.refresh();
    assert!(stage.hears_media());
    let (title, cover) = shown(&mut stage);
    assert!(title == "Night Drive" && cover != 0, "{title} {cover}");
    let first = cover;
    playing::heard(Some(&TRACK.replace("true", "false")), None, 0);
    assert_eq!(shown(&mut stage).1, 0, "a reading without its picture drops the cover");
    playing::heard(Some(TRACK), Some(&[0xff00_00ff_u32 as i32; 16]), 4);
    let (_, again) = shown(&mut stage);
    assert!(again != 0 && again != first, "a new picture is a new cover");
    let previous = playing::now().and_then(|p| p.previous.as_ref().map(|c| c.serial));
    assert_eq!(previous, Some(first), "the cover before stays the previous one");
    playing::allow(false);
    playing::heard(Some(TRACK), None, 0);
    assert_eq!(shown(&mut stage), waiting(), "without notification access nothing is read");
    assert!(
        ask(json!({ "op": "grant", "uid": pulse, "input": "media_network", "on": true }))["error"]
            .is_string()
    );
    assert_eq!(ask(json!({ "op": "grant", "uid": pulse, "input": "media", "on": false })), Value::Null);
    stage.refresh();
    assert!(!stage.hears_media());
}
