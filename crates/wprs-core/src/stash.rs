use serde_json::{Map, Value as Json, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

mod keeper;

pub use keeper::Keeper;

pub const LIMIT: usize = 64 << 10;
pub const EVERY: Duration = Duration::from_secs(5);
const OUTPUTS: usize = 16;
const SCOPES: [&str; 2] = ["package", "output"];

pub struct Stash {
    path: PathBuf,
    package: Map<String, Json>,
    outputs: Map<String, Json>,
    used: Vec<String>,
    version: u64,
    dirty: bool,
    forgotten: bool,
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
    let path = file(dir, package);
    path.iter().for_each(|p| keeper::drop_open(p));
    match path.map(std::fs::remove_file) {
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
        let used = outputs.keys().cloned().collect();
        let (version, dirty, forgotten, written) = (0, false, false, None);
        Some(Stash { path, package, outputs, used, version, dirty, forgotten, written })
    }

    pub fn scopes(&self, output: &str) -> Json {
        json!({ "package": self.package, "output": object(self.outputs.get(output)) })
    }

    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn touch(&mut self, output: &str) {
        if let Some(at) = self.used.iter().position(|o| o == output) {
            let o = self.used.remove(at);
            self.used.push(o);
        }
    }

    pub fn merge(&mut self, output: &str, ops: &Json) -> bool {
        let mut fine = true;
        for at in SCOPES {
            let set = ops.pointer(&format!("/set/{at}")).and_then(Json::as_object).filter(|m| !m.is_empty());
            let gone = ops.pointer(&format!("/gone/{at}")).and_then(Json::as_array).filter(|g| !g.is_empty());
            if set.is_none() && gone.is_none() {
                continue;
            }
            let mut next =
                if at == "package" { self.package.clone() } else { object(self.outputs.get(output)) };
            gone.into_iter().flatten().filter_map(Json::as_str).for_each(|k| drop(next.remove(k)));
            set.into_iter().flatten().for_each(|(k, v)| drop(next.insert(k.clone(), v.clone())));
            if !fits(&next) {
                fine = false;
                continue;
            }
            match at {
                "package" => self.package = next,
                _ => self.place(output, next),
            }
            (self.dirty, self.version) = (true, self.version + 1);
        }
        fine
    }

    fn place(&mut self, output: &str, next: Map<String, Json>) {
        self.used.retain(|o| o != output);
        while self.used.len() >= OUTPUTS {
            let oldest = self.used.remove(0);
            self.outputs.remove(&oldest);
        }
        self.used.push(output.to_owned());
        self.outputs.insert(output.to_owned(), Json::Object(next));
    }

    pub fn due(&self, now: Instant) -> bool {
        self.dirty && !self.forgotten && self.written.is_none_or(|at| now.duration_since(at) >= EVERY)
    }

    pub fn flush(&mut self, now: Instant, force: bool) -> std::io::Result<()> {
        if !self.dirty || self.forgotten || !(force || self.due(now)) {
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
