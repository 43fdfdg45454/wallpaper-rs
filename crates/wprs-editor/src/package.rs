use crate::check::{check_files, review};
use crate::project::Project;
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use wprs_format::{
    Ancestor, Kind, Limits, MANIFEST, Manifest, Origin, OriginKind, SIGNATURE, Signer, sign_source,
};

pub fn problems(dir: &Path, doc: &Value) -> Vec<crate::Problem> {
    let mut all = review(doc, Some(dir));
    for p in check_files(dir, doc) {
        if !all.iter().any(|q| q.message == p.message) {
            all.push(p);
        }
    }
    all
}

pub fn name(doc: &Value) -> String {
    let field = |k: &str| doc["manifest"][k].as_str().unwrap_or("scene").to_owned();
    format!("{}-{}.wpak", field("id"), field("version"))
}

pub fn output(dir: &Path, doc: &Value) -> Result<PathBuf> {
    let dir = fs::canonicalize(dir).with_context(|| format!("cannot find {}", dir.display()))?;
    Ok(dir.parent().unwrap_or(&dir).join(name(doc)))
}

fn unsign(dir: &Path) -> Result<()> {
    match fs::remove_file(dir.join(SIGNATURE)) {
        Err(e) if e.kind() != ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

pub fn build(project: &mut Project, out: &Path, signer: Option<&Signer>) -> Result<(String, u64)> {
    ensure!(!project.dirty(), "save the scene before building it");
    project.settle()?;
    build_saved(&project.dir, &project.doc, out, signer)
}

pub fn build_saved(dir: &Path, doc: &Value, out: &Path, signer: Option<&Signer>) -> Result<(String, u64)> {
    if let Some(p) = problems(dir, doc).into_iter().find(|p| !p.warning) {
        bail!("{}: {}", p.pointer, p.message);
    }
    match signer {
        Some(signer) => drop(sign_source(dir, signer)?),
        None => unsign(dir)?,
    }
    let sha256 = wprs_format::build(dir, out, &Limits::default())?;
    Ok((sha256, fs::metadata(out)?.len()))
}

pub fn extract(file: &Path, dir: &Path) -> Result<String> {
    let empty = !dir.exists() || fs::read_dir(dir)?.next().is_none();
    ensure!(empty, "{} is not an empty folder", dir.display());
    let mut package = wprs_format::open(file, &Limits::default())?;
    ensure!(package.manifest.kind == Kind::Scene, "{} is not a scene package", file.display());
    wprs_format::extract(&mut package, dir)?;
    unsign(dir)?;
    let path = dir.join(MANIFEST);
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path)?)?;
    let id = manifest["id"].as_str().unwrap_or("org.example.scene");
    let id = if id.ends_with(".copy") { id.to_owned() } else { format!("{id}.copy") };
    if let Some(fields) = manifest.as_object_mut().filter(|_| package.manifest.v2()) {
        fields.remove("authors");
        fields.insert("origin".into(), serde_json::to_value(derived(&package.manifest))?);
    }
    manifest["id"] = json!(id);
    fs::write(&path, serde_json::to_string_pretty(&manifest)? + "\n")?;
    Ok(id)
}

fn derived(original: &Manifest) -> Origin {
    let cut = |text: String| text.chars().take(128).collect::<String>();
    let authors = Some(original.authors.join(", ")).filter(|a| !a.trim().is_empty());
    let ancestor = Ancestor {
        author: cut(authors.unwrap_or_else(|| original.id.clone())),
        title: Some(cut(original.title_in(&[]))).filter(|t| !t.trim().is_empty()),
        url: original.links.as_ref().and_then(|l| l.homepage.clone()),
        license: Some(original.license.clone()),
    };
    let earlier = original.origin.iter().flat_map(|o| o.derived_from.iter().cloned()).take(63);
    Origin {
        kind: OriginKind::Derived,
        derived_from: earlier.chain([ancestor]).collect(),
        ..Origin::default()
    }
}

pub fn signer(key: &Path, password: Option<String>) -> Result<Signer> {
    let secret = fs::read_to_string(key).with_context(|| format!("cannot read {}", key.display()))?;
    Ok(Signer::new(&secret, password)?)
}

pub fn applied(doc: &Value) -> Result<PathBuf> {
    let dir = dirs::data_dir().context("there is no data folder")?.join("wallpaper-rs").join("scenes");
    fs::create_dir_all(&dir)?;
    Ok(dir.join(name(doc)))
}
