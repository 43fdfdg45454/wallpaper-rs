use crate::author::Author;
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use wprs_editor::actions::Action;
use wprs_editor::{Op, Project, references};

pub(crate) fn value(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.into()))
}

pub(crate) fn edit(dir: &Path, action: Action) -> Result<()> {
    let mut project = Project::open(dir)?;
    let undoing = matches!(action, Action::Undo {} | Action::Redo {});
    let named = match &action {
        Action::RenameProperty { id, .. } | Action::RemoveProperty { id } => {
            references::scripts(&project.dir, &project.doc, id).into_iter().map(|p| (p, id.clone())).collect()
        }
        _ => Vec::new(),
    };
    let ops = project.act(&action)?;
    ensure!(!(undoing && ops.is_empty()), "there is nothing to {}", action.name());
    project.save()?;
    named.iter().for_each(|(path, id)| eprintln!("warning: {path} names {id:?}; scripts are not rewritten"));
    let added = ops.iter().filter_map(|op| match op {
        Op::Add { path, value } => {
            Some(value["id"].as_str().unwrap_or(path.rsplit('/').next().unwrap_or_default()))
        }
        _ => None,
    });
    let added: Vec<&str> = added.collect();
    match action {
        Action::Import { .. } => added.last().iter().for_each(|id| println!("{id}")),
        Action::AddPreset { .. } => {
            let last = project.doc["manifest"]["presets"].as_array().and_then(|l| l.last());
            last.and_then(|p| p["id"].as_str()).iter().for_each(|id| println!("{id}"))
        }
        Action::AddProperty { .. } => ops
            .iter()
            .filter_map(|op| match op {
                Op::Add { path, .. } => {
                    path.strip_prefix("/manifest/properties/").filter(|p| !p.contains('/'))
                }
                _ => None,
            })
            .for_each(|id| println!("{id}")),
        Action::Thumbnail { .. } => ops
            .iter()
            .filter_map(|op| match op {
                Op::Add { value, .. } | Op::Replace { value, .. } => value.as_str(),
                _ => None,
            })
            .for_each(|path| println!("{path}")),
        _ => ops
            .iter()
            .filter_map(|op| match op {
                Op::Add { value, .. } => value["id"].as_str(),
                _ => None,
            })
            .for_each(|id| println!("{id}")),
    }
    Ok(())
}

pub(crate) fn action(command: Author) -> Result<(PathBuf, Action), Box<Author>> {
    Ok(match command {
        Author::Set { dir, pointer, value: v } => (dir, Action::Set { pointer, value: value(&v) }),
        Author::Add { dir, pointer, value: v } => (dir, Action::Add { pointer, value: value(&v) }),
        Author::Remove { dir, pointer } => (dir, Action::Remove { pointer }),
        Author::Move { dir, from, to } => (dir, Action::Move { from, to }),
        Author::Layers(command) => crate::layering::action(command),
        Author::Import { dir, file, id } => {
            (dir, Action::Import { file: fs::canonicalize(&file).unwrap_or(file), id })
        }
        Author::ImportEffect { dir, descriptor, wgsl } => {
            let wgsl = wgsl.map(|w| fs::canonicalize(&w).unwrap_or(w));
            (
                dir,
                Action::ImportEffect {
                    descriptor: fs::canonicalize(&descriptor).unwrap_or(descriptor),
                    wgsl,
                },
            )
        }
        Author::ImportScript { dir, file } => {
            (dir, Action::ImportScript { file: fs::canonicalize(&file).unwrap_or(file) })
        }
        Author::Translate { dir, pointer, language, text, add, .. } => {
            (dir, Action::Translate { pointer, language, text, add })
        }
        Author::Thumbnail { dir, file, animated } => {
            (dir, Action::Thumbnail { file: fs::canonicalize(&file).unwrap_or(file), animated })
        }
        Author::Upgrade { dir, rating, reasons, photosensitive, derived_from } => {
            let rating = rating.and_then(|r| serde_json::from_value(json!(r)).ok());
            let reasons = reasons.iter().filter_map(|r| serde_json::from_value(json!(r)).ok()).collect();
            (dir, Action::Upgrade { rating, reasons, photosensitive, derived_from })
        }
        Author::Properties(command) => crate::userprops::action(command),
        Author::Undo { dir } => (dir, Action::Undo {}),
        Author::Redo { dir } => (dir, Action::Redo {}),
        other => return Err(Box::new(other)),
    })
}
