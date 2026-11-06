use crate::author::Author;
use crate::flying::{AnimCmd, Flying, PathCmd};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::PathBuf;
use wprs_editor::Project;
use wprs_editor::actions::{Action, AnimEdit, Edit3d, PathEdit, PathKey, PlayEdit, RuleEdit, ShadowEdit};

pub(crate) fn three(text: &str) -> Option<[f32; 3]> {
    let list: Vec<f32> = text.split(',').map(|v| v.trim().parse().ok()).collect::<Option<_>>()?;
    list.try_into().ok()
}

fn wide(text: &Option<String>) -> Result<Option<[f64; 3]>> {
    let Some(t) = text else { return Ok(None) };
    let found = three(t).with_context(|| format!("{t} is not 3 numbers apart by commas"))?;
    Ok(Some(found.map(f64::from)))
}

fn shown(command: &Flying) -> bool {
    match command {
        Flying::Animate { what, .. } => matches!(**what, AnimCmd::Clips | AnimCmd::Bones),
        Flying::Shadows { show, .. } => *show,
        Flying::CameraPath { what, .. } => matches!(what, PathCmd::Show { .. }),
        Flying::NewMaterial { check, .. } => *check,
        Flying::Debug3d { .. } => false,
        Flying::EffectRule { param, .. } => param.is_none(),
        Flying::Clocks { .. } => true,
    }
}

fn animate(layer: String, what: AnimCmd) -> Result<AnimEdit> {
    Ok(match what {
        AnimCmd::Layer {
            add,
            index,
            clip,
            weight,
            rate,
            additive,
            blend_in,
            blend_out,
            enabled,
            bones,
            to,
            remove,
        } => AnimEdit::Layer(PlayEdit {
            layer,
            index,
            add,
            clip,
            remove,
            weight,
            rate,
            additive,
            blend_in,
            blend_out,
            enabled,
            bones,
            to,
        }),
        AnimCmd::Root { on } => AnimEdit::Root { layer, on },
        AnimCmd::Preset { bone, preset } => AnimEdit::Preset { layer, bone, preset },
        AnimCmd::Morph { shape, weight } => {
            let weight = match weight.as_str() {
                "none" => None,
                w => Some(w.parse::<f64>().with_context(|| format!("{w} is not a number"))?),
            };
            AnimEdit::Morph { layer, shape, weight }
        }
        AnimCmd::Clips | AnimCmd::Bones => unreachable!(),
    })
}

fn path(what: PathCmd) -> Result<PathEdit> {
    Ok(match what {
        PathCmd::Add { id } => PathEdit::Add { id },
        PathCmd::Key { path, time, eye, center, up, fov, from, remove } => {
            let given = eye.is_some() || center.is_some() || up.is_some() || fov.is_some();
            let (eye, center, up) = (wide(&eye)?, wide(&center)?, wide(&up)?);
            PathEdit::Key(PathKey {
                path,
                time,
                from,
                eye,
                center,
                up,
                fov,
                from_view: !given && from.is_none(),
                remove,
            })
        }
        PathCmd::Remove { path } => PathEdit::Remove { path },
        PathCmd::Set { path, closed, ease, to } => PathEdit::Set { path, closed, ease, to },
        PathCmd::Order { order } => PathEdit::Order { order },
        PathCmd::Fade { to, duration, clear } => PathEdit::Fade { to, duration, clear },
        PathCmd::Show { .. } => unreachable!(),
    })
}

fn edit(command: Flying) -> Result<(PathBuf, Action)> {
    Ok(match command {
        Flying::Animate { dir, layer, what } => (dir, Action::Animate(animate(layer, *what)?)),
        Flying::Shadows { dir, layer, on, resolution, automatic, bias, .. } => {
            (dir, Action::Scene3d(Edit3d::Shadows(ShadowEdit { layer, on, resolution, bias, automatic })))
        }
        Flying::CameraPath { dir, what } => (dir, Action::Scene3d(Edit3d::Path(path(what)?))),
        Flying::NewMaterial { dir, name, .. } => (dir, Action::Scene3d(Edit3d::NewMaterial { name })),
        Flying::EffectRule { dir, effect, param, uses, waits_for, yields_to } => {
            let param = param.unwrap_or_default();
            (dir, Action::EffectRule(RuleEdit { effect, param, uses, waits_for, yields_to }))
        }
        Flying::Clocks { .. } => anyhow::bail!("clocks only prints"),
        Flying::Debug3d { dir, show } => {
            let show = (show != "none").then_some(show);
            (dir, Action::Scene3d(Edit3d::Debug { show }))
        }
    })
}

pub(crate) fn action(command: Flying) -> Result<(PathBuf, Action), Box<Author>> {
    if shown(&command) {
        return Err(Box::new(Author::Flying(command)));
    }
    edit(command).map_err(|e| {
        eprintln!("wprs-ctl: {e:#}");
        std::process::exit(2)
    })
}

pub(crate) fn run(command: Flying) -> Result<()> {
    let (dir, q) = match command {
        Flying::Animate { dir, layer, what } if matches!(*what, AnimCmd::Clips) => {
            (dir, json!({ "motion": { "layer": layer } }))
        }
        Flying::Animate { dir, layer, .. } => {
            let p = Project::open(&dir)?;
            let known = wprs_editor::rigs::known(&p.dir, &p.doc, &layer);
            anyhow::ensure!(!known.is_null(), "there is no mesh or model layer {layer:?}");
            println!("{}", serde_json::to_string_pretty(&*known)?);
            return Ok(());
        }
        Flying::Shadows { dir, .. } => (dir, json!({ "shadows3d": { "time": 0 } })),
        Flying::CameraPath { dir, what: PathCmd::Show { time } } => {
            (dir, json!({ "paths3d": { "time": time } }))
        }
        Flying::NewMaterial { dir, name, .. } => (dir, json!({ "material_check": { "name": name } })),
        Flying::EffectRule { dir, effect, .. } => (dir, json!({ "rules": { "effect": effect } })),
        Flying::Clocks { dir, time, strip } => (dir, json!({ "clocks": { "time": time, "strip": strip } })),
        other => anyhow::bail!("{other:?} changes the scene"),
    };
    let p = Project::open(&dir)?;
    let found: Value = wprs_editor::rigs::query(&p.dir, &p.seen(), "", &q);
    anyhow::ensure!(!found.is_null(), "there is nothing to show");
    println!("{}", serde_json::to_string_pretty(&found)?);
    if found["ok"] == false {
        anyhow::bail!("the shader does not build: {}", found["error"].as_str().unwrap_or_default());
    }
    Ok(())
}
