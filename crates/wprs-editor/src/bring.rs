use crate::actions::ImportAssets;
use crate::layers::taken;
use crate::ops::Op;
use crate::shelf::{NAMED, Pick, files, fragment};
use crate::stock::{credit, opened};
use anyhow::{Result, bail, ensure};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::path::Path;
use wprs_core::message::said;
use wprs_format::{SCHEMA, check_path, sha256};

fn ids(fragment: &Value, out: &mut Vec<(String, String)>) {
    fn layers(list: &Value, out: &mut Vec<(String, String)>) {
        for layer in list.as_array().into_iter().flatten() {
            out.extend(layer["id"].as_str().map(|id| ("layers".into(), id.into())));
            layers(&layer["layers"], out);
        }
    }
    layers(&fragment["layers"], out);
    for section in NAMED {
        out.extend(
            fragment[section].as_object().into_iter().flatten().map(|(k, _)| (section.into(), k.clone())),
        );
    }
    let timelines = fragment["timelines"].as_array().into_iter().flatten();
    out.extend(timelines.filter_map(|t| Some(("timelines".into(), t["id"].as_str()?.into()))));
}

fn free(doc: &Value, section: &str, id: &str, used: &BTreeSet<String>) -> bool {
    let scene = &doc["scene"];
    let own = match section {
        "layers" | "assets" | "particles" => taken(doc, id),
        "timelines" => scene["timelines"].as_array().into_iter().flatten().any(|t| t["id"] == id),
        _ => scene[section].get(id).is_some(),
    };
    !own && !used.contains(id)
}

fn renamed(value: &Value, names: &BTreeMap<String, String>, top: bool) -> Value {
    match value {
        Value::String(s) => json!(names.get(s).unwrap_or(s)),
        Value::Array(items) => Value::Array(items.iter().map(|v| renamed(v, names, false)).collect()),
        Value::Object(map) => {
            let key =
                |k: &String, inner: bool| if inner { names.get(k).unwrap_or(k).clone() } else { k.clone() };
            let mut out = Map::new();
            for (k, v) in map {
                let inner = top && NAMED.contains(&k.as_str());
                let v = match v.as_object().filter(|_| inner) {
                    Some(m) => Value::Object(
                        m.iter().map(|(n, x)| (key(n, true), renamed(x, names, false))).collect(),
                    ),
                    None if k == "text" => v.clone(),
                    None => renamed(v, names, false),
                };
                out.insert(k.clone(), v);
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

fn prefixed(fragment: &mut Value, prefix: &str) {
    let at = |v: &mut Value| {
        if let Some(path) = v.as_str() {
            *v = json!(format!("{prefix}{path}"));
        }
    };
    fragment["assets"].as_object_mut().into_iter().flatten().for_each(|(_, a)| at(&mut a["path"]));
    fragment["scripts"].as_array_mut().into_iter().flatten().for_each(|s| at(&mut s["path"]));
    for (_, effect) in fragment["effects"].as_object_mut().into_iter().flatten() {
        at(&mut effect["descriptor"]);
        at(&mut effect["wgsl"]);
    }
}

pub fn import(dir: &Path, doc: &Value, action: &ImportAssets) -> Result<Vec<Op>> {
    ensure!(
        doc["manifest"]["schema"] == SCHEMA,
        said("editor.credits_need_v2", "upgrade the scene to the second version before importing parts", &[])
    );
    let (mut package, shared, offered) = opened(&action.package)?;
    let picks: Vec<Pick> = match action.parts.is_empty() {
        true => offered.iter().map(|p| Pick { kind: p.kind.clone(), id: p.id.clone() }).collect(),
        false => action.parts.iter().map(|t| Pick::parse(t)).collect::<Result<_>>()?,
    };
    for pick in &picks {
        if !offered.iter().any(|p| p.kind == pick.kind && p.id == pick.id) {
            bail!(said("editor.part_not_offered", "the package does not offer %1", &[&pick.key()]));
        }
    }
    let mut part = fragment(&shared, &picks)?;
    let (mut names, mut used, mut found) = (BTreeMap::new(), BTreeSet::new(), Vec::new());
    ids(&part, &mut found);
    for (section, id) in found {
        let candidates = (1..10_000).map(|n| if n == 1 { id.clone() } else { format!("{id}_{n}") });
        let Some(new) = candidates.into_iter().find(|c| free(doc, &section, c, &used)) else {
            bail!(said("editor.id_use", "the id %1 is in use", &[&id]))
        };
        used.insert(new.clone());
        if new != id {
            names.insert(id, new);
        }
    }
    part = renamed(&part, &names, true);
    let m = package.manifest.clone();
    let prefix = format!("assets/{}-{}/", m.id, m.version);
    let mut groups: BTreeMap<(String, String, Option<String>), Vec<String>> = BTreeMap::new();
    for path in files(&part) {
        let target = format!("{prefix}{path}");
        check_path(&target)?;
        let bytes = package.read_file(&path)?;
        let disk = dir.join(&target);
        match fs::read(&disk) {
            Ok(old) if sha256(&old[..])? != sha256(&bytes[..])? => {
                bail!(said("editor.shared_differs", "%1 exists with other content", &[&target]))
            }
            Ok(_) => {}
            Err(_) => {
                fs::create_dir_all(disk.parent().unwrap_or(dir))?;
                std::io::Write::write_all(&mut File::create(&disk)?, &bytes)?;
            }
        }
        groups.entry(credit(&m, &path)).or_default().push(target);
    }
    prefixed(&mut part, &prefix);
    let mut ops = crate::credit::credited(doc, &m, groups)?;
    ops.extend(crate::credit::placed(doc, &part));
    Ok(ops)
}
