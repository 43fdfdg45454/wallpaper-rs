use crate::author::Author;
use crate::modeling::{GroupCmd, ModelCmd, Modeling};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::PathBuf;
use wprs_editor::Project;
use wprs_editor::actions::{
    Action, CameraEdit, Edit3d, MaterialEdit, ModelEdit, ModelSet, TonesEdit, ViewEdit,
};

fn numbers<const N: usize>(text: &Option<String>) -> Result<Option<[f64; N]>> {
    let Some(t) = text else { return Ok(None) };
    let list: Vec<f64> =
        t.split(',').map(|v| v.trim().parse::<f64>()).collect::<Result<_, _>>().ok().unwrap_or_default();
    let found: Option<[f64; N]> = list.try_into().ok();
    found.map(Some).with_context(|| format!("{t} is not {N} numbers apart by commas"))
}

fn color(text: Option<String>) -> Option<Value> {
    text.map(|t| if t == "none" { Value::Null } else { crate::steps::value(&t) })
}

fn model(what: ModelCmd) -> Result<Edit3d> {
    Ok(Edit3d::Model(match what {
        ModelCmd::Add { layer, asset, into, at } => {
            ModelEdit::Add { asset, id: Some(layer), into, at: numbers(&at)? }
        }
        ModelCmd::Set { layer, position, rotation, scale, size, env, asset } => ModelEdit::Set(ModelSet {
            layer,
            position: numbers(&position)?,
            rotation: numbers(&rotation)?,
            scale: numbers(&scale)?,
            size: numbers(&size)?,
            env,
            asset,
        }),
        ModelCmd::Remove { layer } => ModelEdit::Remove { layer },
        ModelCmd::Info { .. } => unreachable!(),
    }))
}

fn shown(command: &Modeling) -> bool {
    matches!(
        command,
        Modeling::Model { what: ModelCmd::Info { .. }, .. }
            | Modeling::Group3d { what: GroupCmd::Show { .. }, .. }
            | Modeling::Material { list: true, .. }
            | Modeling::ModelInfo { .. }
            | Modeling::Cost3d { .. }
    )
}

fn edit(command: Modeling) -> Result<(PathBuf, Edit3d)> {
    Ok(match command {
        Modeling::Model { dir, what } => (dir, model(what)?),
        Modeling::Group3d { dir, what } => (
            dir,
            match what {
                GroupCmd::Add { id, into, layers } => Edit3d::Group { id: Some(id), into, layers },
                GroupCmd::Space { layer, off } => Edit3d::Space { layer, deep: !off },
                GroupCmd::DepthTest { layer, off } => Edit3d::DepthTest { layer, on: !off },
                GroupCmd::Show { .. } => unreachable!(),
            },
        ),
        Modeling::Material {
            dir,
            layer,
            name,
            shader,
            lit,
            double_sided,
            cast_shadows,
            receive_shadows,
            color: c,
            emissive,
            params,
            clear,
            ..
        } => {
            let mut set = std::collections::BTreeMap::new();
            for p in params {
                let (id, v) = p.split_once('=').with_context(|| format!("{p} is not ID=VALUE"))?;
                set.insert(
                    id.trim().to_owned(),
                    v.trim().parse::<f64>().with_context(|| format!("{v} is not a number"))?,
                );
            }
            let name = name.context("name the material, or use --list")?;
            let m = MaterialEdit {
                layer,
                name,
                shader,
                lit,
                double_sided,
                cast_shadows,
                receive_shadows,
                color: color(c),
                emissive: color(emissive),
                params: set,
                clear,
            };
            (dir, Edit3d::Material(m))
        }
        Modeling::Camera3d { dir, fov, near, far, eye, center, up, roll, from_view, clear } => {
            let (eye, center, up) = (numbers(&eye)?, numbers(&center)?, numbers(&up)?);
            (dir, Edit3d::Camera(CameraEdit { fov, near, far, eye, center, up, roll, from_view, clear }))
        }
        Modeling::View3d { dir, orbit, pan, zoom, frame, through, reset } => (
            dir,
            Edit3d::View(ViewEdit {
                orbit: numbers(&orbit)?,
                pan: numbers(&pan)?,
                zoom,
                through,
                frame,
                reset,
            }),
        ),
        Modeling::Tonemap { dir, hdr, map, split } => {
            (dir, Edit3d::Tones(TonesEdit { hdr, tonemap: map, split }))
        }
        Modeling::Scene3d { dir, model, group, space, depth_test, material, camera, view, tones } => {
            let named = [("model", model), ("group", group), ("space", space), ("depth_test", depth_test)];
            let more = [("material", material), ("camera", camera), ("view", view), ("tones", tones)];
            let given: Vec<(&str, String)> =
                named.into_iter().chain(more).filter_map(|(k, v)| Some((k, v?))).collect();
            anyhow::ensure!(given.len() == 1, "give exactly one of the 3D edits");
            let (key, text) = &given[0];
            let value: Value = serde_json::from_str(text).with_context(|| format!("{text} is not JSON"))?;
            (dir, serde_json::from_value(json!({ *key: value }))?)
        }
        Modeling::ModelInfo { .. } | Modeling::Cost3d { .. } => unreachable!(),
    })
}

pub(crate) fn action(command: Modeling) -> Result<(PathBuf, Action), Box<Author>> {
    if shown(&command) {
        return Err(Box::new(Author::Modeling(command)));
    }
    match edit(command) {
        Ok((dir, e)) => Ok((dir, Action::Scene3d(e))),
        Err(e) => {
            eprintln!("wprs-ctl: {e:#}");
            std::process::exit(2);
        }
    }
}

pub(crate) fn run(command: Modeling) -> Result<()> {
    let ask = |dir: &PathBuf, q: Value| -> Result<Value> {
        let p = Project::open(dir)?;
        Ok(wprs_editor::rigs::query(&p.dir, &p.doc, "", &q))
    };
    let found = match command {
        Modeling::ModelInfo { file } => {
            let (model, head) = wprs_editor::stage3d::file(&file)?;
            wprs_editor::stage3d::info(&model, &head)
        }
        Modeling::Model { dir, what: ModelCmd::Info { layer } } => {
            let p = Project::open(&dir)?;
            let asset = wprs_editor::actions::layer(&p.doc, &layer)
                .ok()
                .and_then(|(_, l)| l["model"].as_str().map(String::from));
            wprs_editor::stage3d::model_info(&p.dir, &p.doc, &json!({ "asset": asset.unwrap_or(layer) }))
        }
        Modeling::Group3d { dir, what: GroupCmd::Show { time } } => {
            ask(&dir, json!({ "stage3d": { "time": time } }))?
        }
        Modeling::Material { dir, layer, .. } => ask(&dir, json!({ "materials3d": { "layer": layer } }))?,
        Modeling::Cost3d { dir, quality } => {
            ask(&dir, json!({ "cost3d": { "quality": quality.unwrap_or("q3".into()) } }))?
        }
        other => anyhow::bail!("{other:?} changes the scene"),
    };
    anyhow::ensure!(!found.is_null(), "there is nothing to show");
    anyhow::ensure!(found.get("error").is_none(), "{}", found["error"].as_str().unwrap_or_default());
    println!("{}", serde_json::to_string_pretty(&found)?);
    Ok(())
}
