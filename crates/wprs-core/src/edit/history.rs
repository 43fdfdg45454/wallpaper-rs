use super::Edit;
use crate::store::write_private;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const STEPS: usize = 100;
const MAX_BYTES: usize = 8 << 20;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    pub undo: Vec<Edit>,
    pub redo: Vec<Edit>,
}

impl History {
    pub fn place(image: &Path) -> PathBuf {
        let base = dirs::state_dir().or_else(dirs::data_dir).unwrap_or_else(|| PathBuf::from("."));
        base.join("wallpaper-rs").join("edit-history").join(super::file::key(image) + ".json")
    }

    pub fn load(image: &Path) -> History {
        let text = std::fs::read_to_string(Self::place(image)).unwrap_or_default();
        let read: History = serde_json::from_str(&text).unwrap_or_default();
        let fine = |e: &Edit| e.validate().is_ok();
        History {
            undo: read.undo.into_iter().filter(fine).collect(),
            redo: read.redo.into_iter().filter(fine).collect(),
        }
    }

    pub fn save(&mut self, image: &Path) -> Result<(), String> {
        let path = Self::place(image);
        if self.undo.is_empty() && self.redo.is_empty() {
            return match std::fs::remove_file(&path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            };
        }
        let mut text = serde_json::to_string(self).map_err(|e| e.to_string())?;
        while text.len() > MAX_BYTES && !(self.undo.is_empty() && self.redo.is_empty()) {
            if self.undo.is_empty() {
                self.redo.remove(0)
            } else {
                self.undo.remove(0)
            };
            text = serde_json::to_string(self).map_err(|e| e.to_string())?;
        }
        write_private(&path, text.as_bytes()).map_err(|e| e.to_string())
    }

    pub fn record(&mut self, before: &Edit, after: &Edit) {
        if before == after {
            return;
        }
        self.undo.push(before.clone());
        if self.undo.len() > STEPS {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self, current: &Edit) -> Option<Edit> {
        let back = self.undo.pop()?;
        self.redo.push(current.clone());
        Some(back)
    }

    pub fn redo(&mut self, current: &Edit) -> Option<Edit> {
        let again = self.redo.pop()?;
        self.undo.push(current.clone());
        Some(again)
    }
}

impl Edit {
    pub fn before(&self) -> Edit {
        let (geometry, fit, source) = (self.geometry.clone(), self.fit, self.source.clone());
        Edit {
            geometry,
            fit,
            source,
            outputs: self.outputs.clone(),
            span: self.span.clone(),
            ..Edit::default()
        }
    }
}
