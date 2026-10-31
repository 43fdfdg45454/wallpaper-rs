#[path = "../common/mod.rs"]
mod common;

use common::sources;

mod inputs;
mod limits;

fn load(source: &str) -> Script {
    plain(&[("main.js", source)], 1).unwrap()
}
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use wprs_scene::{AudioFrame, Course, Fired, LayerOverride, PointerState, ScriptOutputs, TimelineCommand};
use wprs_script::{Error, Frame, Limits, Script, Setup};

fn update(script: &mut Script, time: f64) -> Result<ScriptOutputs, Error> {
    let props = [("glow".to_string(), json!(0.5))].into();
    let audio = AudioFrame { bass: 0.25, beat: true, ..AudioFrame::default() };
    let pointer =
        PointerState { pos: Some([0.25, 0.75]), buttons: 1, inside: true, ..PointerState::default() };
    script.update(&Frame {
        dt: 0.016,
        props: &props,
        audio: Some(&audio),
        pointer: &pointer,
        ..Frame::at(time)
    })
}

fn layer(outputs: &ScriptOutputs, id: &str) -> LayerOverride {
    outputs.layers[id].clone()
}

fn plain(list: &[(&str, &str)], seed: u64) -> Result<Script, Error> {
    Script::load(&Setup { seed, locale: "en".into(), ..Setup::plain(&sources(list)) }, Limits::default())
}
