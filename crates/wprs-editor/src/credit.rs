use crate::ops::{Op, escape, set};
use crate::shelf::{LISTED, NAMED};
use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use wprs_core::message::said;
use wprs_format::Manifest;

type Groups = BTreeMap<(String, String, Option<String>), Vec<String>>;

fn cut(text: &str) -> String {
    text.chars().take(128).collect()
}

pub fn credited(doc: &Value, m: &Manifest, groups: Groups) -> Result<Vec<Op>> {
    let mut list: Vec<Value> = doc["manifest"]["credits"].as_array().cloned().unwrap_or_default();
    let source = json!({ "id": m.id, "version": m.version });
    for ((license, attribution, url), paths) in groups {
        let same = |c: &Value| {
            c["license"] == license.as_str()
                && c["attribution"] == attribution.as_str()
                && c["url"].as_str() == url.as_deref()
                && c["source"] == source
                && c["paths"].as_array().map_or(0, Vec::len) + paths.len() <= 256
        };
        match list.iter_mut().find(|c| same(c)) {
            Some(old) => {
                let held = old["paths"].as_array_mut().into_iter().flatten();
                let mut all: Vec<Value> = held.map(|p| p.clone()).collect();
                all.extend(
                    paths.iter().filter(|p| !all.contains(&json!(p))).map(|p| json!(p)).collect::<Vec<_>>(),
                );
                old["paths"] = json!(all);
            }
            None => {
                for chunk in paths.chunks(256) {
                    let mut credit = json!({ "paths": chunk, "license": license, "attribution": attribution,
                        "source": source });
                    if let Some(url) = &url {
                        credit["url"] = json!(url);
                    }
                    list.push(credit);
                }
            }
        }
    }
    ensure!(
        list.len() <= 64,
        said("editor.credits_most", "a package has at most 64 credits; credit folders instead of files", &[])
    );
    let mut ops = set(doc, "/manifest/credits", json!(list));
    let origin = &doc["manifest"]["origin"];
    let kind = origin["kind"].as_str().unwrap_or("original");
    if kind == "original" || kind == "derived" {
        let authors = Some(m.authors.join(", ")).filter(|a| !a.trim().is_empty()).unwrap_or(m.id.clone());
        let mut ancestor =
            json!({ "author": cut(&authors), "title": cut(&m.title_in(&[])), "license": m.license });
        if let Some(url) = m.links.as_ref().and_then(|l| l.homepage.clone()) {
            ancestor["url"] = json!(url);
        }
        let mut from: Vec<Value> = origin["derived_from"].as_array().cloned().unwrap_or_default();
        if !from.iter().any(|a| a["author"] == ancestor["author"] && a["title"] == ancestor["title"]) {
            from.push(ancestor);
        }
        ensure!(
            from.len() <= 64,
            said("editor.derived_most", "a package is derived from at most 64 works", &[])
        );
        let mut next = json!({ "kind": "derived", "derived_from": from });
        if let Some(url) = origin.get("source_url") {
            next["source_url"] = url.clone();
        }
        ops.extend(set(doc, "/manifest/origin", next));
    }
    Ok(ops)
}

pub fn placed(doc: &Value, part: &Value) -> Vec<Op> {
    let mut ops = Vec::new();
    for section in NAMED {
        let Some(entries) = part[section].as_object().filter(|e| !e.is_empty()) else { continue };
        let at = format!("/scene/{section}");
        match doc.pointer(&at) {
            None => ops.extend(set(doc, &at, json!(entries))),
            Some(_) => ops.extend(
                entries
                    .iter()
                    .map(|(k, v)| Op::Add { path: format!("{at}/{}", escape(k)), value: v.clone() }),
            ),
        }
    }
    for section in LISTED {
        let Some(items) = part[section].as_array().filter(|e| !e.is_empty()) else { continue };
        let at = format!("/scene/{section}");
        match doc.pointer(&at).and_then(Value::as_array) {
            None => ops.extend(set(doc, &at, json!(items))),
            Some(old) => ops.extend(
                items
                    .iter()
                    .enumerate()
                    .map(|(i, v)| Op::Add { path: format!("{at}/{}", old.len() + i), value: v.clone() }),
            ),
        }
    }
    ops
}
