use std::collections::BTreeMap;
use wprs_scene::*;

mod shaders;
mod valid;

const SAMPLE: &str = include_str!("../sample.json");

fn props() -> BTreeMap<String, Property> {
    serde_json::from_str(include_str!("../props.json")).unwrap()
}

fn sample() -> Scene {
    Scene::from_json(SAMPLE).unwrap()
}

fn issues(edit: impl FnOnce(&mut Scene)) -> Vec<Issue> {
    let mut scene = sample();
    edit(&mut scene);
    scene.validate(&props())
}
