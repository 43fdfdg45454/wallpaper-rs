use crate::meta::facts_of;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use wprs_core::message::{Message, said};
use wprs_format::features::{check, lacks, notes, playable};
use wprs_format::{Error, Facts, Manifest, Platform};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Problem {
    pub pointer: String,
    pub message: String,
    pub warning: bool,
    pub said: Message,
}

pub(crate) fn problem(pointer: impl Into<String>, said: Message) -> Problem {
    Problem { pointer: pointer.into(), message: said.to_string(), warning: false, said }
}

pub(crate) fn warning(pointer: impl Into<String>, said: Message) -> Problem {
    Problem { warning: true, ..problem(pointer, said) }
}

pub(crate) fn manifest_problem(e: Error) -> Problem {
    match &e {
        Error::Invalid { field, reason } => {
            let pointer = format!("/manifest/{}", field.split('.').collect::<Vec<_>>().join("/"));
            problem(pointer, reason.put(said("editor.field", "%1: %2", &[field, reason]), 2))
        }
        e => problem("/manifest", e.message()),
    }
}

fn at(field: &str) -> String {
    match field {
        f if f.starts_with("inputs") => "/manifest/inputs",
        f if f.starts_with("the scene costs") => "/scene/layers",
        _ => "/manifest/requires/features",
    }
    .into()
}

pub fn problems(doc: &Value, dir: Option<&Path>) -> Vec<Problem> {
    let Some((manifest, found)) = facts_of(doc, dir).filter(|(m, _)| m.v2()) else { return Vec::new() };
    let mut out: Vec<Problem> = check(&manifest, &found)
        .err()
        .into_iter()
        .map(|e| match e {
            Error::Invalid { field, reason } => Problem {
                warning: field != "inputs",
                ..problem(at(&field), reason.put(said("editor.field", "%1: %2", &[&field, &reason]), 2))
            },
            e => problem("/manifest", e.message()),
        })
        .collect();
    let unread = dir.map(|d| manifest.unread_texts(wprs_format::locale_names(d))).unwrap_or_default();
    let unread = unread.into_iter().map(|n| said("format.note", "%1", &[&n]));
    let notes = notes(&manifest, &found).into_iter().chain(unread);
    out.extend(notes.map(|note| warning(at(&note.to_string()), note)));
    out.extend(verdicts(&manifest, &found).into_iter().map(|v| warning("/manifest/requires/features", v)));
    out
}

pub fn verdicts(manifest: &Manifest, found: &Facts) -> Vec<Message> {
    let verdict = |p: Platform| match (playable(manifest, found, p), lacks(p.name(), &found.features)) {
        (Err(e), _) => Some(said("editor.refused", "%1: refused: %2", &[&p.name(), &e]).with(2, e.message())),
        (Ok(()), without) if !without.is_empty() => {
            Some(said("editor.plays_without", "%1: plays without %2", &[&p.name(), &without.join(", ")]))
        }
        _ => None,
    };
    Platform::ALL.into_iter().filter_map(verdict).collect()
}

pub fn told(error: &anyhow::Error) -> String {
    told_with(error, &wprs_core::message::tr)
}

pub fn told_with(error: &anyhow::Error, tr: &dyn Fn(&str) -> String) -> String {
    let one = |e: &(dyn std::error::Error + 'static)| {
        let message = e.downcast_ref::<Message>().cloned();
        let message = message.or_else(|| e.downcast_ref::<Error>().map(Error::message));
        let message =
            message.or_else(|| e.downcast_ref::<wprs_scene::Issue>().map(wprs_scene::Issue::message));
        message.map_or_else(|| e.to_string(), |m| m.told(tr))
    };
    error.chain().map(one).collect::<Vec<_>>().join(": ")
}
