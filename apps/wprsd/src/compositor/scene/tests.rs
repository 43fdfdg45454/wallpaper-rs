use super::super::Content;
use super::super::scripts::Scripts;
use super::*;
use crate::scenes::ScenePack;
use std::sync::Arc;
use wprs_scene::{Scene, Tier};

#[test]
fn new_media_of_the_same_scene_keep_its_clock_and_view() {
    let renderer = match Renderer::shared() {
        Ok(renderer) => renderer,
        Err(e) => return assert!(std::env::var_os("WPRS_REQUIRE_GPU").is_none(), "GPU required: {e}"),
    };
    let scene = Scene::from_json(include_str!("../../../../../scenes/snowfall/scene.json")).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../../../scenes/snowfall/manifest.json")).unwrap();
    let properties = serde_json::from_value(manifest["properties"].clone()).unwrap();
    let pack = Arc::new(ScenePack::bare(scene, properties, Vec::new()));
    let spec =
        SceneSpec { values: pack.props.values(&[]), pack, extra: Vec::new(), clips: Vec::new(), hold: None };
    let extra = vec![("photo".to_string(), wprs_render::Pixels::new(image::RgbaImage::new(2, 2)))];
    let turned = SceneSpec { extra, ..spec.clone() };
    let (old, new) = (Content::Scene(spec.clone()), Content::Scene(turned.clone()));
    assert!(old.same_source(&new) && !old.same_look(&new));
    let power = Power::new(Tier::T3, false, 0);
    let (mut state, players, start) = (SceneState::new(spec, power), HashMap::new(), Instant::now());
    let at = |s: u64| start + Duration::from_secs(s);
    for s in [0, 2] {
        state.render(renderer, ((16, 9), 60), power, &players, true, at(s)).unwrap();
    }
    let time = state.runtime.time();
    assert!(time > 1.0 && state.update(turned));
    state.render(renderer, ((16, 9), 60), power, &players, true, at(3)).unwrap();
    assert!(state.view.is_some() && state.runtime.time() > time, "the scene restarted");
}

#[test]
fn screens_showing_one_scene_share_its_script_process() {
    let scene = Scene::from_json(include_str!("../../../../../scenes/orbits/scene.json")).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../../../scenes/orbits/manifest.json")).unwrap();
    let properties = serde_json::from_value(manifest["properties"].clone()).unwrap();
    let mut pack = ScenePack::bare(scene, properties, Vec::new());
    pack.scripts = vec![(
        "scripts/orbits.js".into(),
        include_str!("../../../../../scenes/orbits/scripts/orbits.js").into(),
    )];
    let pack = Arc::new(pack);
    let power = Power::new(Tier::T3, false, 0);
    let spec = |values: Values| SceneSpec {
        values,
        pack: pack.clone(),
        extra: Vec::new(),
        clips: Vec::new(),
        hold: None,
    };
    let mut scripts = Scripts::default();
    let mut states: Vec<SceneState> =
        (0..3).map(|_| SceneState::new(spec(pack.props.values(&[])), power)).collect();
    let mut other = pack.props.values(&[]);
    other.insert("other".into(), serde_json::json!(1));
    states.push(SceneState::new(spec(other), power));
    states.iter_mut().for_each(|s| s.link(&mut scripts));
    let shares = |states: &[SceneState]| states.iter().map(|s| s.shares().0).collect::<Vec<_>>();
    assert_eq!(shares(&states), [3, 3, 3, 1]);
    let mut changed = pack.props.values(&[]);
    changed.insert("other".into(), serde_json::json!(2));
    states[0].update(spec(changed.clone()));
    states[0].link(&mut scripts);
    assert_eq!(shares(&states), [1, 2, 2, 1], "a screen with other settings kept the shared process");
    let alone = states[3].shares().1;
    changed.insert("other".into(), serde_json::json!(3));
    states[3].update(spec(changed));
    states[3].link(&mut scripts);
    assert_eq!(states[3].shares(), (1, alone), "a screen alone restarted its process");
    states.truncate(1);
    assert_eq!(states[0].shares().0, 1);
}

#[test]
fn declared_inputs_without_scripts_wait_for_what_the_scene_binds() {
    let scene = Scene::from_json(include_str!("../../../../../scenes/dusk/scene.json")).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_str(include_str!("../../../../../scenes/dusk/manifest.json")).unwrap();
    let properties = serde_json::from_value(manifest["properties"].clone()).unwrap();
    let mut pack = ScenePack::bare(scene, properties, Vec::new());
    pack.inputs = Some(Uses { cursor: true, scripts: true, ..Uses::default() });
    let pack = Arc::new(pack);
    let power = Power::new(Tier::T3, false, 0);
    let follows = |depth: &str| {
        let mut values = pack.props.values(&[]);
        values.insert("depth".into(), serde_json::json!(depth));
        let spec = SceneSpec { values, pack: pack.clone(), extra: Vec::new(), clips: Vec::new(), hold: None };
        let mut state = SceneState::new(spec, power);
        let pointer = PointerState { pos: Some([0.5, 0.5]), inside: true, ..PointerState::default() };
        let moved = state.input(SceneInput::Pointer(String::new(), pointer, Vec::new(), [true; 2]));
        (state.uses().cursor, state.uses().audio, moved)
    };
    assert_eq!(follows("pointer"), (true, false, true));
    assert_eq!(follows("orbit"), (false, false, false));
    assert_eq!(follows("off"), (false, false, false));
}
