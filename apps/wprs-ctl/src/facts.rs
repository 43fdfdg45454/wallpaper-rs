use crate::show::{self, list, text};
use anyhow::Result;
use serde_json::{Value, json};
use std::path::Path;
use wprs_format::{Content, Error, Facts, Limits, Manifest, Origin, features, inspect, open, upgrade};

pub fn check(path: &Path, json: bool) -> Result<()> {
    let (manifest, facts) = match path.is_dir() {
        true => inspect(path)?,
        false => open(path, &Limits::default()).map(|p| (p.manifest, p.facts))?,
    };
    match features::check(&manifest, &facts) {
        Err(Error::Invalid { field, .. }) if field == "requires.features" => {}
        checked if manifest.v2() => checked?,
        _ => {}
    }
    let listed = manifest.features();
    let missing = facts.features.iter().filter(|f| manifest.v2() && !listed.contains(f));
    let why = |f: &String| json!({"name": f, "why": features::explain(f)});
    let report = json!({
        "path": path, "schema": manifest.schema, "id": manifest.id, "version": manifest.version,
        "kind": manifest.kind, "title": manifest.title_in(&wprs_core::locale::languages()),
        "rating": manifest.content.as_ref().map(|c| text(&c.rating)),
        "reasons": manifest.content.iter().flat_map(|c| &c.descriptors).map(text).collect::<Vec<_>>(),
        "photosensitive": manifest.photosensitive, "origin": manifest.origin.as_ref().map(|o| text(&o.kind)),
        "inputs": manifest.inputs.iter().map(|i| i.name()).collect::<Vec<_>>(),
        "features": facts.features.iter().map(why).collect::<Vec<_>>(), "declared": listed,
        "missing": missing.map(why).collect::<Vec<_>>(),
        "facets": facts.facets, "languages": manifest.locales(),
        "warnings": features::warnings(&manifest, &facts),
    });
    match json {
        true => println!("{report}"),
        false => print!("{}", human(&manifest, &facts, &report)),
    }
    Ok(())
}

fn human(manifest: &Manifest, facts: &Facts, report: &Value) -> String {
    let words = |key: &str| list(report[key].as_array().into_iter().flatten().filter_map(Value::as_str));
    let named =
        |f: &Value| format!("{} ({})", f["name"].as_str().unwrap_or(""), f["why"].as_str().unwrap_or(""));
    let needs = |key: &str| list(report[key].as_array().into_iter().flatten().map(named));
    let flag = |on: Option<bool>| on.map_or("unknown", |on| if on { "yes" } else { "no" }).to_string();
    let rating = report["rating"].as_str().unwrap_or("unrated").to_string();
    let reasons = Some(words("reasons")).filter(|r| !r.is_empty()).map(|r| format!(" ({r})"));
    let head = format!("ok {} {} {} ({})\n", manifest.kind, manifest.id, manifest.version, manifest.schema);
    let pairs = [
        ("title", report["title"].as_str().unwrap_or("").to_string()),
        ("rating", rating + &reasons.unwrap_or_default()),
        ("photosensitive", flag(manifest.photosensitive)),
        ("origin", report["origin"].as_str().unwrap_or("-").to_string()),
        ("inputs", words("inputs")),
        ("needs", needs("features")),
        ("build adds", needs("missing")),
        ("facets", list(&facts.facets)),
        ("languages", words("languages")),
    ];
    let shown = pairs.into_iter().filter(|(_, v)| !v.is_empty());
    let warnings = report["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str);
    head + &show::lines(shown) + &warnings.map(|w| format!("warning: {w}\n")).collect::<String>()
}

pub fn upgraded(
    dir: &Path,
    rating: &str,
    reasons: &[String],
    photosensitive: bool,
    from: &[String],
) -> Result<()> {
    let content: Content = serde_json::from_value(json!({"rating": rating, "descriptors": reasons}))?;
    let (manifest, renames) = upgrade(dir, content, photosensitive, Origin::by(from))?;
    let inputs = list(manifest.inputs.iter().map(|i| i.name()));
    println!("{} {} is now {} (inputs: {})", manifest.id, manifest.version, manifest.schema, inputs);
    renames.iter().for_each(|(old, new)| println!("property {old} is now {new}"));
    Ok(())
}
