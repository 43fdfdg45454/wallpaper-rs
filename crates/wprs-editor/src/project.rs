use crate::actions::{Action, plan};
use crate::history::History;
use crate::meta::{cancels, detected, sync};
use crate::ops::Op;
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use wprs_core::message::said;
use wprs_format::{MANIFEST, check_path, sha256};

pub struct Project {
    pub dir: PathBuf,
    pub doc: Value,
    pub history: History,
    pub base: String,
    pub pending: Option<(Value, History)>,
}

pub fn stamp(doc: &Value) -> String {
    sha256(doc.to_string().as_bytes()).unwrap_or_default()
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

pub fn entry(doc: &Value) -> Result<String> {
    let entry = doc["manifest"]["entry"].as_str().unwrap_or("scene.json").to_owned();
    check_path(&entry)?;
    ensure!(
        entry != MANIFEST && entry.ends_with(".json"),
        said("editor.entry_not_scene", "the entry %1 is not a scene file", &[&entry])
    );
    Ok(entry)
}

pub fn read(dir: &Path) -> Result<Value> {
    let load = |name: &str| -> Result<Value> {
        let text = fs::read_to_string(dir.join(name)).with_context(|| format!("cannot read {name}"))?;
        serde_json::from_str(&text).with_context(|| format!("{name} is not valid JSON"))
    };
    let manifest = load(MANIFEST)?;
    let scene = load(&entry(&json!({"manifest": manifest}))?)?;
    Ok(json!({"manifest": manifest, "scene": scene}))
}

pub(crate) fn write(path: &Path, text: &str, mode: u32) -> Result<()> {
    let dir = path.parent().context("a file needs a folder")?;
    let tmp = dir.join(format!(".{}.tmp", path.file_name().unwrap_or_default().to_string_lossy()));
    let mut file = fs::OpenOptions::new().write(true).create(true).truncate(true).mode(mode).open(&tmp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(fs::rename(tmp, path)?)
}

impl Project {
    pub fn open(dir: &Path) -> Result<Project> {
        let doc = read(dir)?;
        let base = stamp(&doc);
        let (history, pending) = match crate::session::saved(dir).filter(|s| s.files == base) {
            Some(s) if s.doc == doc => (s.history, None),
            Some(s) => (History::default(), Some((s.doc, s.history))),
            None => (History::default(), None),
        };
        let mut project = Project { dir: dir.to_owned(), doc, history, base, pending };
        if project.history.done.is_empty() && project.history.saved.is_none() {
            project.history.mark_saved();
        }
        Ok(project)
    }

    pub fn create(dir: &Path, doc: Value) -> Result<Project> {
        let empty = !dir.exists() || fs::read_dir(dir)?.next().is_none();
        ensure!(empty, said("editor.not_empty_folder", "%1 is not an empty folder", &[&(dir.display())]));
        fs::create_dir_all(dir.join("assets"))?;
        let (history, base) = (History::default(), String::new());
        let mut project = Project { dir: dir.to_owned(), doc, history, base, pending: None };
        project.write_files()?;
        project.history.mark_saved();
        project.persist()?;
        Ok(project)
    }

    fn write_files(&mut self) -> Result<()> {
        let pretty = |v: &Value| serde_json::to_string_pretty(v).map(|t| t + "\n");
        write(&self.dir.join(MANIFEST), &pretty(&self.doc["manifest"])?, 0o644)?;
        write(&self.dir.join(entry(&self.doc)?), &pretty(&self.doc["scene"])?, 0o644)?;
        self.base = stamp(&self.doc);
        Ok(())
    }

    pub fn act(&mut self, action: &Action) -> Result<Vec<Op>> {
        self.step(action, false)
    }

    pub fn drag(&mut self, action: &Action) -> Result<Vec<Op>> {
        self.step(action, true)
    }

    fn step(&mut self, action: &Action, gesture: bool) -> Result<Vec<Op>> {
        self.ready()?;
        let ops = match action {
            Action::Undo {} => {
                return Ok(self.history.undo(&mut self.doc)?.map(|(_, o)| o).unwrap_or_default());
            }
            Action::Redo {} => {
                return Ok(self.history.redo(&mut self.doc)?.map(|(_, o)| o).unwrap_or_default());
            }
            Action::Import { file, id } => {
                let (id, path) = crate::assets::import(&self.dir, &self.doc, file, id.as_deref())?;
                crate::assets::adopt(&self.doc, &id, &path)?
            }
            Action::ImportEffect { descriptor, wgsl } => {
                crate::shaders::import(&self.dir, &self.doc, descriptor, wgsl.as_deref())?
            }
            Action::ImportScript { file } => crate::scripts::import(&self.dir, &self.doc, file)?,
            Action::Thumbnail { file, animated } => {
                let path = crate::previews::store(&self.dir, &self.doc, file, *animated)?;
                crate::previews::adopt(&self.doc, *animated, &path)?
            }
            action => plan(&self.doc, action)?,
        };
        self.commit(&action.name(), ops, gesture)
    }

    pub fn effects(&self) -> crate::inspect::Own {
        let Ok(mut scene) = serde_json::from_value::<wprs_scene::Scene>(self.doc["scene"].clone()) else {
            return Default::default();
        };
        crate::cache::read_files(&mut scene, &self.dir);
        scene.loaded
    }

    pub fn record(&mut self, label: &str, ops: Vec<Op>) -> Result<Vec<Op>> {
        self.ready()?;
        self.commit(label, ops, false)
    }

    fn commit(&mut self, label: &str, ops: Vec<Op>, gesture: bool) -> Result<Vec<Op>> {
        let dir = Some(self.dir.as_path());
        if ops.is_empty() || cancels(&self.doc, &ops, dir) {
            return Ok(Vec::new());
        }
        let before = detected(&self.doc, dir);
        let mut done = self.history.record(&mut self.doc, label, ops, now(), gesture)?;
        let follow = sync(&self.doc, &before, dir);
        if !follow.is_empty() {
            done.extend(self.history.extend(&mut self.doc, follow)?);
        }
        Ok(done)
    }

    pub fn settle(&mut self) -> Result<bool> {
        let follow = sync(&self.doc, &Default::default(), Some(&self.dir));
        if follow.is_empty() || self.dirty() || self.pending.is_some() {
            return Ok(false);
        }
        self.record("sync", follow)?;
        self.save().map(|_| true)
    }

    fn ready(&self) -> Result<()> {
        ensure!(
            self.pending.is_none(),
            said(
                "editor.unsaved_changes_earlier",
                "unsaved changes of an earlier session wait; recover or discard them first",
                &[]
            )
        );
        Ok(())
    }

    pub fn save(&mut self) -> Result<()> {
        self.ready()?;
        let disk = read(&self.dir).map(|d| stamp(&d)).unwrap_or_default();
        if disk != self.base {
            bail!(said(
                "editor.files_changed_outside",
                "the files of %1 changed outside the editor; open the folder again",
                &[&(self.dir.display())]
            ));
        }
        self.write_files()?;
        self.history.mark_saved();
        self.persist()
    }

    pub fn dirty(&self) -> bool {
        stamp(&self.doc) != self.base
    }
}
