use crate::author::Author;
use crate::lamping::{Lamp, Lamping, Lit};
use anyhow::Result;
use serde_json::{Value, json};
use std::path::PathBuf;
use wprs_editor::Project;
use wprs_editor::actions::{Action, AmbientEdit, LightEdit, LightSet, LightingEdit, MapStroke, NormalsMake};

fn pair(text: &str) -> Option<[f64; 2]> {
    text.split_once(',').and_then(|(x, y)| Some([x.trim().parse().ok()?, y.trim().parse().ok()?]))
}

fn parsed<T: serde::de::DeserializeOwned>(text: &str) -> Option<T> {
    serde_json::from_value(json!(text)).ok()
}

fn color(text: Option<String>) -> Option<Value> {
    text.map(|t| crate::steps::value(&t))
}

fn light(layer: String, what: Lamp) -> LightEdit {
    match what {
        Lamp::Add { light, into, at } => LightEdit::Add {
            light: parsed(&light).unwrap_or_default(),
            id: Some(layer),
            into,
            at: at.as_deref().and_then(pair),
        },
        Lamp::Set(s) => LightEdit::Set(LightSet {
            layer,
            light: s.light.as_deref().and_then(parsed),
            color: color(s.color),
            intensity: s.intensity,
            radius: s.radius,
            z: s.z,
            cone: s.cone.as_deref().and_then(pair),
            length: s.length,
            at: s.at.as_deref().and_then(pair),
            aim: s.aim.as_deref().and_then(pair),
            tilt: s.tilt,
        }),
        Lamp::Remove => LightEdit::Remove { layer },
        Lamp::Solo { off } => LightEdit::Solo { layer: (!off).then_some(layer) },
        Lamp::Mute { off } => LightEdit::Mute { layer, muted: !off },
        Lamp::Volumetric(a) => crate::glowing::air(layer, &a),
        Lamp::Projection(c) => crate::glowing::cast(layer, &c),
        Lamp::Falloff { .. } => unreachable!(),
    }
}

fn lighting(l: Lit) -> (PathBuf, Action) {
    let e = LightingEdit {
        layer: l.layer,
        normal: l.normal,
        env: l.env,
        roughness_map: l.roughness_map,
        emissive_map: l.emissive_map,
        roughness: l.roughness,
        reflection: l.reflection,
        emissive: l.emissive,
        strength: l.strength,
        off: l.off,
    };
    (l.dir, Action::Lighting(e))
}

fn shown(what: &Lamping) -> bool {
    match what {
        Lamping::Light { what: Lamp::Volumetric(a), .. } => a.show,
        Lamping::Light { what: Lamp::Projection(c), .. } => c.show,
        Lamping::Light { what, .. } => matches!(what, Lamp::Falloff { .. }),
        Lamping::Lights { .. } => true,
        Lamping::Normals { preview, generate, .. } => *preview && !*generate,
        Lamping::PaintMap { heat, .. } | Lamping::Ambient { preview: heat, .. } => *heat,
        Lamping::Lighting(_) => false,
    }
}

pub(crate) fn action(command: Lamping) -> Result<(PathBuf, Action), Box<Author>> {
    if shown(&command) {
        return Err(Box::new(Author::Lamping(command)));
    }
    Ok(match command {
        Lamping::Light { dir, layer, what } => (dir, Action::Light(light(layer, what))),
        Lamping::Lighting(l) => lighting(l),
        Lamping::Normals { dir, asset, strength, blur, layer, id, .. } => {
            (dir, Action::Normals(NormalsMake { asset: Some(asset), layer, strength, blur, id }))
        }
        Lamping::PaintMap { dir, layer, map, points, radius, hardness, value, .. } => {
            let points = points.unwrap_or_default();
            let points = points.split(';').filter(|p| !p.trim().is_empty()).map(pair);
            let points = points.map(|p| p.map(|[x, y]| [x as f32, y as f32])).collect::<Option<Vec<_>>>();
            let map = parsed(&map).unwrap_or(wprs_editor::actions::Map::Roughness);
            let stroke =
                MapStroke { layer, map, radius, hardness, value, points: points.unwrap_or_default() };
            (dir, Action::PaintMap(stroke))
        }
        Lamping::Ambient { dir, ambient, sky, clear, .. } => {
            (dir, Action::Ambient(AmbientEdit { ambient: color(ambient), sky: color(sky), clear }))
        }
        Lamping::Lights { .. } => unreachable!(),
    })
}

pub(crate) fn run(command: Lamping) -> Result<()> {
    let open = |dir: &PathBuf| Project::open(dir);
    let found = match command {
        Lamping::Light { dir, layer, what: Lamp::Volumetric(a) } => {
            return crate::glowing::print(&dir, |p| wprs_editor::lamps::air(&p.dir, &p.doc, &layer, a.time));
        }
        Lamping::Light { dir, layer, what: Lamp::Projection(c) } => {
            return crate::glowing::print(&dir, |p| wprs_editor::lamps::cast(&p.dir, &p.doc, &layer, c.time));
        }
        Lamping::Light { dir, layer, what: Lamp::Falloff { time } } => {
            let p = open(&dir)?;
            wprs_editor::lamps::falloff(&p.dir, &p.doc, &layer, time)
        }
        Lamping::Lights { dir, time, cost: true, quality } => {
            let p = open(&dir)?;
            let quality = quality.as_deref().and_then(parsed).unwrap_or_default();
            wprs_editor::lamps::cost(&p.dir, &p.doc, time, quality)
        }
        Lamping::Lights { dir, time, .. } => {
            let p = open(&dir)?;
            wprs_editor::lamps::lights(&p.dir, &p.doc, time)
        }
        Lamping::Normals { dir, asset, strength, blur, .. } => {
            let p = open(&dir)?;
            let m = NormalsMake { asset: Some(asset), layer: None, strength, blur, id: None };
            wprs_editor::lamps::normals_preview(&p.dir, &p.doc, &m)
        }
        Lamping::PaintMap { dir, layer, map, .. } => {
            let p = open(&dir)?;
            wprs_editor::lamps::heat(&p.dir, &p.doc, &layer, &format!("{map}_map"))
        }
        Lamping::Ambient { dir, .. } => wprs_editor::lamps::hemisphere(&open(&dir)?.doc),
        other => anyhow::bail!("{other:?} changes the scene"),
    };
    anyhow::ensure!(!found.is_null(), "there is nothing to show");
    println!("{}", serde_json::to_string_pretty(&found)?);
    Ok(())
}
