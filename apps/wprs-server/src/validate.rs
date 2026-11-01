use crate::App;
use crate::error::{Problem, Res};
use anyhow::{Context, ensure};
use image::codecs::jpeg::JpegEncoder;
use image::{ImageReader, Limits as Pixels};
use rusqlite::params;
use rustix::process::{Resource, Rlimit, setrlimit};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Cursor;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use wprs_format::{Kind, Limits, Rating, SIGNATURE, fingerprint, public_key, verify_package};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub id: String,
    pub version: String,
    pub kind: Kind,
    pub title: String,
    pub description: String,
    pub license: String,
    pub tags: Vec<String>,
    pub authors: Vec<String>,
    pub signer: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub photosensitive: Option<bool>,
    #[serde(default)]
    pub facets: Vec<String>,
}

pub fn check(wpak: &Path, keys: &[String], require_signature: bool, thumb: &Path) -> anyhow::Result<Report> {
    for (resource, value) in [(Resource::As, 1 << 30), (Resource::Cpu, 60)] {
        let _ = setrlimit(resource, Rlimit { current: Some(value), maximum: Some(value) });
    }
    let mut package = wprs_format::open(wpak, &Limits::default())?;
    package.manifest.publishable()?;
    package.look_closer()?;
    let pairs = keys
        .iter()
        .map(|k| Ok((fingerprint(k)?, public_key(k)?)))
        .collect::<wprs_format::Result<Vec<_>>>()?;
    let pairs: Vec<(&str, &str)> = pairs.iter().map(|(fp, key)| (fp.as_str(), key.as_str())).collect();
    let signed = package.entries.iter().any(|e| e.path == SIGNATURE);
    ensure!(signed || !require_signature, "the repository only accepts packages signed by their author");
    let unknown = "the manifest signature matches no author key of your account that the repository accepts";
    let signer = signed.then(|| verify_package(&mut package, &pairs).context(unknown)).transpose()?;
    let rating = package.manifest.content.as_ref().map(|c| c.rating);
    let preview =
        package.manifest.thumbnail().cloned().context("the server needs a preview image in the package")?;
    let mut reader = ImageReader::new(Cursor::new(package.read_file(&preview)?)).with_guessed_format()?;
    let mut limits = Pixels::default();
    (limits.max_image_width, limits.max_image_height, limits.max_alloc) =
        (Some(16384), Some(16384), Some(512 << 20));
    reader.limits(limits);
    let image = reader.decode().context("the preview is not a PNG, JPEG, WebP, GIF or BMP image")?;
    JpegEncoder::new_with_quality(File::create(thumb)?, 80)
        .encode_image(&image.thumbnail(640, 640).to_rgb8())?;
    let (m, mut facets) = (package.manifest.localized(), package.facts.facets);
    if rating == Some(Rating::Mature) {
        facets.push("mature".into());
        facets.sort();
    }
    let (title, description) = (m.title_in(&[]), m.pick(&m.description, &[]));
    Ok(Report {
        id: m.id,
        version: m.version,
        kind: m.kind,
        title,
        description,
        license: m.license,
        tags: m.tags,
        authors: m.authors,
        signer,
        content: rating.map(|r| if r == Rating::General { "general" } else { "mature" }.into()),
        photosensitive: m.photosensitive,
        facets,
    })
}

pub async fn validate(app: &App, user: i64, wpak: &Path, thumb: &Path) -> Res<Report> {
    let _permit = app.validations.acquire().await.map_err(Problem::internal)?;
    let sql = "SELECT fingerprint, public_key FROM author_keys WHERE user_id = ?1";
    let keys = app.db.run(move |c| {
        let mut statement = c.prepare(sql)?;
        let rows =
            statement.query_map(params![user], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    });
    let mut keys = keys.await?;
    let roots = app.repo.authors();
    if let Some((root_keys, _, false)) = &roots {
        keys.retain(|(fp, _)| root_keys.contains(fp));
    }
    let mut command = tokio::process::Command::new(&app.cfg.validator);
    command.arg("validate").arg(wpak).arg("--thumb").arg(thumb).env_clear().kill_on_drop(true);
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    for (_, key) in &keys {
        command.arg("--key").arg(key);
    }
    if roots.is_some_and(|(_, require, _)| require) {
        command.arg("--require-signature");
    }
    let run = tokio::time::timeout(Duration::from_secs(120), command.output()).await;
    let output = run.map_err(|_| Problem::bad("validation took too long"))??;
    let stopped = || Problem::bad("validation stopped: the package needs too much memory or time");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|_| stopped())?;
    if !output.status.success() {
        return Err(Problem::bad(value["error"].as_str().unwrap_or("invalid package").to_string()));
    }
    Ok(serde_json::from_value(value)?)
}
