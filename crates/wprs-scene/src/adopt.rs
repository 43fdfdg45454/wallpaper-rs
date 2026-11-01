use crate::custom::{MAX_WGSL, MAX_WGSL_TOTAL};
use crate::effects::Descriptor;
use crate::validate::Issue;
use crate::{Property, Scene, SceneError};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Display;
use std::sync::{Arc, Mutex};

mod models;

pub trait Files {
    type Error: Display;

    fn read(&mut self, path: &str, max: usize) -> Result<Vec<u8>, Self::Error>;

    fn head(&mut self, path: &str, max: usize, keep: usize) -> Result<(Vec<u8>, u64), Self::Error> {
        let mut all = self.read(path, max)?;
        let size = all.len() as u64;
        all.truncate(keep);
        Ok((all, size))
    }
}

impl<E: Display, F: FnMut(&str, usize) -> Result<Vec<u8>, E>> Files for F {
    type Error = E;

    fn read(&mut self, path: &str, max: usize) -> Result<Vec<u8>, E> {
        self(path, max)
    }
}

type Compiled = Result<Arc<Descriptor>, String>;

static COMPILED: Mutex<BTreeMap<[u8; 32], Compiled>> = Mutex::new(BTreeMap::new());

fn compiled(id: &str, json: &str, wgsl: &str) -> Compiled {
    let key: [u8; 32] = Sha256::new()
        .chain_update(id)
        .chain_update([0])
        .chain_update(json)
        .chain_update([0])
        .chain_update(wgsl)
        .finalize()
        .into();
    let mut cache = COMPILED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(found) = cache.get(&key) {
        return found.clone();
    }
    let found = Descriptor::packaged(id, json, wgsl).map(Arc::new);
    if found.as_ref().is_err_and(|e| e.contains(crate::STOPPED) || e.contains(crate::UNCHECKED)) {
        return found;
    }
    if cache.len() >= 64 {
        cache.clear();
    }
    cache.insert(key, found.clone());
    found
}

impl Scene {
    pub fn adopt(&mut self, props: &BTreeMap<String, Property>, files: impl Files) -> Result<(), SceneError> {
        self.read_files(files)?;
        self.validate(props).into_iter().next().map_or(Ok(()), |issue| Err(SceneError::Invalid(issue)))
    }

    pub fn read_files(&mut self, mut files: impl Files) -> Result<(), SceneError> {
        let fail = |path: String, reason: String| SceneError::Invalid(Issue::Effect { path, reason });
        let mut shaders = 0;
        for (id, effect) in self.effects.clone() {
            let at = format!("/effects/{id}");
            let mut text = |path: &str| {
                let bytes =
                    files.read(path, MAX_WGSL).map_err(|e| fail(at.clone(), format!("{path}: {e}")))?;
                String::from_utf8(bytes).map_err(|_| fail(at.clone(), format!("{path} is not UTF-8")))
            };
            let (json, wgsl) = (text(&effect.descriptor)?, text(&effect.wgsl)?);
            shaders += wgsl.len();
            if shaders > MAX_WGSL_TOTAL {
                let reason = format!("the shaders of the package pass {} KiB", MAX_WGSL_TOTAL >> 10);
                return Err(fail(at, reason));
            }
            self.loaded.insert(id.clone(), compiled(&id, &json, &wgsl).map_err(|e| fail(at, e))?);
        }
        self.read_models(&mut files, fail)
    }
}
