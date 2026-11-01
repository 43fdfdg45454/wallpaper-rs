use serde_json::{Map, Value as Json, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const LIMIT: usize = 64 << 10;
pub const EVERY: Duration = Duration::from_secs(5);
const OUTPUTS: usize = 16;

pub struct Stash {
    path: PathBuf,
    package: Map<String, Json>,
    outputs: Map<String, Json>,
    dirty: bool,
    written: Option<Instant>,
}

pub fn dir() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("wallpaper-rs").join("storage"))
}

pub fn file(dir: &Path, package: &str) -> Option<PathBuf> {
    let fine = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-');
    (!package.is_empty() && !package.starts_with('.') && package.len() <= 255 && package.chars().all(fine))
        .then(|| dir.join(format!("{package}.json")))
}

pub fn forget(dir: &Path, package: &str) -> std::io::Result<()> {
    match file(dir, package).map(std::fs::remove_file) {
        Some(Err(e)) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

fn object(v: Option<&Json>) -> Map<String, Json> {
    v.and_then(Json::as_object).cloned().unwrap_or_default()
}

fn fits(v: &Map<String, Json>) -> bool {
    serde_json::to_vec(v).is_ok_and(|b| b.len() <= LIMIT)
}

impl Stash {
    pub fn open(dir: &Path, package: &str) -> Option<Stash> {
        let path = file(dir, package)?;
        let read: Json =
            std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let (package, outputs) = (object(read.get("package")), object(read.get("outputs")));
        let package = if fits(&package) { package } else { Map::new() };
        Some(Stash { path, package, outputs, dirty: false, written: None })
    }

    pub fn scopes(&self, output: &str) -> Json {
        json!({ "package": self.package, "output": object(self.outputs.get(output)) })
    }

    pub fn take(&mut self, output: &str, changed: &Json) {
        if let Some(p) = changed.get("package").and_then(Json::as_object).filter(|p| fits(p)) {
            (self.package, self.dirty) = (p.clone(), true);
        }
        if let Some(o) = changed.get("output").and_then(Json::as_object).filter(|o| fits(o)) {
            self.outputs.remove(output);
            if self.outputs.len() >= OUTPUTS {
                let first = self.outputs.keys().next().cloned().unwrap_or_default();
                self.outputs.remove(&first);
            }
            self.outputs.insert(output.to_owned(), Json::Object(o.clone()));
            self.dirty = true;
        }
    }

    pub fn due(&self, now: Instant) -> bool {
        self.dirty && self.written.is_none_or(|at| now.duration_since(at) >= EVERY)
    }

    pub fn flush(&mut self, now: Instant, force: bool) -> std::io::Result<()> {
        if !self.dirty || !(force || self.due(now)) {
            return Ok(());
        }
        let dir = self.path.parent().unwrap_or(Path::new("."));
        std::os::unix::fs::DirBuilderExt::mode(std::fs::DirBuilder::new().recursive(true), 0o700)
            .create(dir)?;
        let mut part = tempfile::NamedTempFile::new_in(dir)?;
        let body = json!({ "package": self.package, "outputs": self.outputs });
        part.write_all(&serde_json::to_vec(&body).unwrap_or_default())?;
        std::fs::set_permissions(part.path(), std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
        part.persist(&self.path).map_err(|e| e.error)?;
        (self.dirty, self.written) = (false, Some(now));
        Ok(())
    }
}

impl Drop for Stash {
    fn drop(&mut self) {
        if let Err(e) = self.flush(Instant::now(), true) {
            tracing::warn!("cannot keep what a scene script stored: {e}");
        }
    }
}
