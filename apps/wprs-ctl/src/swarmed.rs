use crate::author::Author;
use crate::steps::value;
use crate::swarming::{Particle, Remap, Swarming};
use anyhow::Result;
use serde_json::json;
use std::path::PathBuf;
use wprs_editor::Project;
use wprs_editor::actions::{Action, ParticleEdit as P};

fn pair(p: &str) -> Option<[f64; 2]> {
    p.split_once(',').and_then(|(x, y)| Some([x.trim().parse().ok()?, y.trim().parse().ok()?]))
}

fn parsed<T: serde::de::DeserializeOwned>(text: &str) -> Option<T> {
    serde_json::from_value(json!(text)).ok()
}

fn edit(system: String, what: Particle) -> P {
    let layered = |l: Option<String>| l.filter(|l| !l.is_empty());
    match what {
        Particle::Shape { emitter, shape } => P::Shape { system, emitter, shape: value(&shape) },
        Particle::ControlPoint { point, at, angle, cursor, remove } => {
            let point = point[1..].parse().unwrap_or(0);
            let follow = if cursor { Some(true) } else { at.as_ref().map(|_| false) };
            P::Point { system, point, at: at.as_deref().and_then(pair), follow, angle, remove }
        }
        Particle::Remap(Remap::Add { remap }) => {
            P::Remap { system, index: None, remap: Some(value(&remap)), remove: false }
        }
        Particle::Remap(Remap::Set { index, remap }) => {
            P::Remap { system, index: Some(index), remap: Some(value(&remap)), remove: false }
        }
        Particle::Remap(Remap::Remove { index }) => {
            P::Remap { system, index: Some(index), remap: None, remove: true }
        }
        Particle::Cutout { range, layer } => {
            let parts: Vec<f64> = range.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            let range = <[f64; 3]>::try_from(parts.as_slice()).ok().filter(|_| range != "none");
            P::Cutout { system, layer: layered(layer), range }
        }
        Particle::Orient { orient, angle, layer } => {
            P::Orient { system, layer: layered(layer), orient: parsed(&orient), angle }
        }
        Particle::Perspective { focal, depth, depth_speed, off } => {
            let (depth, depth_speed) =
                (depth.as_deref().and_then(pair), depth_speed.as_deref().and_then(pair));
            P::Perspective { system, focal, depth, depth_speed, off }
        }
        Particle::EmitFromLayer { emitter, layer, threshold, color, inherit, jitter, refresh } => {
            let refresh = refresh.as_deref().and_then(parsed);
            P::Layer { system, emitter, id: layer, threshold, color, inherit, jitter, refresh }
        }
        Particle::Inherit { child, take, follow } => P::Inherit {
            system,
            child,
            inherit: take.iter().filter_map(|t| parsed(t)).collect(),
            follow: Some(follow),
        },
        Particle::Shader { shader, layer } => {
            P::Shader { system, layer: layered(layer), shader: Some(shader).filter(|s| s != "none") }
        }
        Particle::Handles { .. } => unreachable!(),
    }
}

pub(crate) fn action(command: Swarming) -> Result<(PathBuf, Action), Box<Author>> {
    match command {
        Swarming::Particles { what: what @ Particle::Handles { .. }, dir, system } => {
            Err(Box::new(Author::Swarming(Swarming::Particles { dir, system, what })))
        }
        Swarming::Particles { dir, system, what } => Ok((dir, Action::Particles(edit(system, what)))),
    }
}

pub(crate) fn run(command: Swarming) -> Result<()> {
    let Swarming::Particles { dir, system, what: Particle::Handles { layer, time } } = command else {
        return Ok(());
    };
    let project = Project::open(&dir)?;
    let found = wprs_editor::swarms::handles(&project.dir, &project.doc, &system, layer.as_deref(), time);
    anyhow::ensure!(!found.is_null(), "there is no particle system {system:?}");
    println!("{}", serde_json::to_string_pretty(&found)?);
    Ok(())
}
