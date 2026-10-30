use crate::assets::{files, unused};
use crate::layers::find;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use wprs_format::{Error, FileEntry, Limits, Property, SIGNATURE, validate};
use wprs_scene::{Layer, Scene, System, Timeline};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Problem {
    pub pointer: String,
    pub message: String,
    pub warning: bool,
}

fn problem(pointer: impl Into<String>, message: impl ToString) -> Problem {
    Problem { pointer: pointer.into(), message: message.to_string(), warning: false }
}

fn manifest_problem(e: Error) -> Problem {
    match e {
        Error::Invalid { field, reason } => {
            let pointer = format!("/manifest/{}", field.split('.').collect::<Vec<_>>().join("/"));
            problem(pointer, format!("{field}: {reason}"))
        }
        e => problem("/manifest", e),
    }
}

fn locate(scene: &Value) -> String {
    fn bad<T: serde::de::DeserializeOwned>(v: &Value) -> bool {
        serde_json::from_value::<T>(v.clone()).is_err()
    }
    fn layer(list: &Value, base: &str) -> Option<String> {
        let (i, l) = list.as_array()?.iter().enumerate().find(|(_, l)| bad::<Layer>(l))?;
        let path = format!("{base}/{i}");
        Some(layer(&l["layers"], &format!("{path}/layers")).unwrap_or(path))
    }
    let entries =
        |key: &str| scene[key].as_object().into_iter().flatten().map(|(k, v)| (k.clone(), v.clone()));
    let system = entries("particles").find(|(_, v)| bad::<System>(v)).map(|(k, _)| format!("/particles/{k}"));
    let timelines = scene["timelines"].as_array().into_iter().flatten().enumerate();
    let timeline =
        timelines.clone().find(|(_, t)| bad::<Timeline>(t)).map(|(i, _)| format!("/timelines/{i}"));
    let found = layer(&scene["layers"], "/layers").or(system).or(timeline);
    format!("/scene{}", found.unwrap_or_default())
}

fn pointer(doc: &Value, path: &str) -> String {
    if path.starts_with("/properties") {
        return format!("/manifest{path}");
    }
    let named = path.strip_prefix("/layers/").map(|rest| rest.split_once('/').unwrap_or((rest, "")));
    match named
        .filter(|(id, _)| id.parse::<usize>().is_err())
        .and_then(|(id, rest)| Some((find(doc, id).ok()?, rest)))
    {
        Some((layer, "")) => layer,
        Some((layer, rest)) => format!("{layer}/{rest}"),
        None => format!("/scene{path}"),
    }
}

pub fn check(doc: &Value) -> Vec<Problem> {
    review(doc, None)
}

pub fn review(doc: &Value, dir: Option<&Path>) -> Vec<Problem> {
    let mut out = Vec::new();
    let props: BTreeMap<String, Property> = match crate::meta::read(doc, dir) {
        Ok(manifest) => {
            if manifest.kind != wprs_format::Kind::Scene {
                out.push(problem("/manifest/kind", "the kind of a scene package is scene"));
            }
            out.extend(manifest.missing().into_iter().map(manifest_problem));
            let first = manifest.check().err().map(manifest_problem);
            out.extend(first.filter(|p| !out.contains(p)));
            manifest.properties
        }
        Err(e) => {
            out.push(problem("/manifest", e));
            BTreeMap::new()
        }
    };
    match serde_json::from_value::<Scene>(doc["scene"].clone()) {
        Err(e) => out.push(problem(locate(&doc["scene"]), e)),
        Ok(mut scene) => {
            let unread = dir.and_then(|d| crate::cache::read_files(&mut scene, d)).into_iter();
            let issues = unread.chain(scene.validate(&props)).map(|issue| {
                let text = issue.to_string();
                let path = text.split_whitespace().next().filter(|p| p.starts_with('/')).unwrap_or("");
                problem(pointer(doc, path.trim_end_matches(':')), text)
            });
            issues.for_each(|p| {
                if !out.contains(&p) {
                    out.push(p)
                }
            });
        }
    }
    out.extend(crate::meta::problems(doc, dir));
    out
}

pub fn check_files(dir: &Path, doc: &Value) -> Vec<Problem> {
    let mut out = Vec::new();
    let found = files(dir);
    let assets = doc.pointer("/scene/assets").and_then(Value::as_object).into_iter().flatten();
    for (id, asset) in assets {
        let path = asset["path"].as_str().unwrap_or_default();
        if !found.iter().any(|(p, _)| p == path) {
            out.push(problem(format!("/scene/assets/{id}/path"), format!("{path} is not in the folder")));
        }
    }
    if let Ok(manifest) = crate::meta::read(doc, Some(dir)) {
        let entries: Vec<FileEntry> = found
            .iter()
            .map(|(path, size)| FileEntry {
                path: path.clone(),
                size: *size,
                compressed_size: *size,
                symlink: false,
            })
            .collect();
        if let Err(e) = validate(&manifest, &entries, &Limits::default()) {
            out.push(manifest_problem(e));
        }
        let previews = manifest.previews.unwrap_or_default();
        for (animated, path) in [(false, previews.thumbnail), (true, previews.animated)] {
            let inside = path.as_ref().filter(|p| found.iter().any(|(f, _)| f == *p));
            let data = inside.and_then(|p| std::fs::read(dir.join(p)).ok());
            if let (Some(path), Some(data)) = (path, data)
                && let Err(e) = wprs_format::media::preview(animated, &path, &data)
            {
                out.push(manifest_problem(e));
            }
        }
    }
    if found.iter().any(|(p, _)| p == SIGNATURE) {
        out.push(Problem {
            pointer: "/manifest".into(),
            message: format!("{SIGNATURE} signs an earlier version; a build without a key removes it"),
            warning: true,
        });
    }
    let scene = serde_json::from_value::<Scene>(doc["scene"].clone()).ok();
    let read = |path: &str| std::fs::read_to_string(dir.join(path)).ok();
    if let Some((i, message)) = scene.and_then(|s| s.undeclared(read)) {
        out.push(Problem { pointer: format!("/scene/scripts/{i}"), message, warning: false });
    }
    let unused = unused(dir, doc).into_iter().map(|path| Problem {
        pointer: "/scene/assets".into(),
        message: format!("{path} is not used by the scene but goes into the package"),
        warning: true,
    });
    out.extend(unused);
    out
}
