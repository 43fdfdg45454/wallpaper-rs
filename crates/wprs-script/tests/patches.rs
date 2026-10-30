mod common;

use common::sources;
use serde_json::Value;
use wprs_scene::TimelineCommand;
use wprs_script::{Encoder, Error, Frame, Limits, Script, Setup};

const SOURCE: &str = r#"import { scene, timeline } from "wprs";
    const moon = scene.layer("moon");
    const shared = { n: 0 };
    export function update({ time }) {
        if (time >= 2) moon.opacity = 0.5;
        if (time >= 3) shared.n = 1;
        if (time >= 4) timeline("intro").play();
        if (time === 6) for (;;) {}
        return shared;
    }"#;

fn sent(script: &mut Script, encoder: &mut Encoder, time: f64) -> Result<Vec<String>, Error> {
    let text = script.run(encoder.encode(&Frame::at(time)))?;
    let patch: Option<Value> = text.map(|t| serde_json::from_str(&t).unwrap());
    Ok(patch.and_then(|p| p.as_object().cloned()).map_or(Vec::new(), |p| p.keys().cloned().collect()))
}

#[test]
fn scripts_send_only_the_outputs_that_changed_and_every_event() {
    let setup = Setup::plain(&sources(&[("main.js", SOURCE)]));
    let mut script = Script::load(&setup, Limits::default()).unwrap();
    let encoder = &mut Encoder::default();
    let first = sent(&mut script, encoder, 0.0).unwrap();
    assert!(
        ["values", "layers", "timelines", "busy", "watch"].iter().all(|k| first.contains(&k.to_string()))
    );
    assert_eq!(sent(&mut script, encoder, 1.0).unwrap(), Vec::<String>::new());
    assert_eq!(
        sent(&mut script, encoder, 2.0).unwrap(),
        ["layer_edits"],
        "only the layer that changed is sent"
    );
    assert_eq!(sent(&mut script, encoder, 3.0).unwrap(), ["values"], "a value changed in place is noticed");
    assert_eq!(sent(&mut script, encoder, 4.0).unwrap(), ["timelines"]);
    assert_eq!(sent(&mut script, encoder, 5.0).unwrap(), ["timelines"], "the same event is sent again");
    assert!(matches!(sent(&mut script, encoder, 6.0), Err(Error::Timeout(_))));
    let again = sent(&mut script, encoder, 7.0).unwrap();
    assert!(again.len() >= first.len(), "after a failure everything is sent again: {again:?}");
}

#[test]
fn merged_outputs_keep_what_did_not_change() {
    let setup = Setup::plain(&sources(&[("main.js", SOURCE)]));
    let mut script = Script::load(&setup, Limits::default()).unwrap();
    let outs: Vec<_> = [0.0, 2.0, 3.0, 4.0, 5.0].map(|t| script.update(&Frame::at(t)).unwrap()).into();
    let last = &outs[4];
    assert_eq!((last.layers["moon"].opacity, last.values["main.js"]["n"].as_i64()), (Some(0.5), Some(1)));
    assert_eq!(last.timelines, [("intro".to_string(), TimelineCommand::Play)]);
    assert!(outs[1].timelines.is_empty() && outs[1].values["main.js"]["n"] == 0);
}
