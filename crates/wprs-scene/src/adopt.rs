use crate::custom::{MAX_WGSL, MAX_WGSL_TOTAL};
use crate::effects::Descriptor;
use crate::gltf::{MAX_MODEL_BYTES, MAX_MODEL_ENTRIES, MAX_MODELS_JSON, ModelFile};
use crate::validate::Issue;
use crate::{Layer, Property, Scene, SceneError};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt::Display;
use std::sync::{Arc, Mutex};

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
        let figures: Vec<String> = self
            .layers()
            .chain(self.templates.values())
            .filter_map(Layer::figure)
            .map(String::from)
            .collect();
        let (mut read, mut json, mut left) =
            (BTreeMap::<String, Arc<ModelFile>>::new(), 0, MAX_MODEL_ENTRIES);
        for id in figures {
            let Some(asset) = self.assets.get(&id).filter(|a| a.extension() == "glb") else { continue };
            let (path, at) = (asset.path.clone(), format!("/assets/{id}"));
            if let Some(file) = read.get(&path) {
                self.models.insert(id, file.clone());
                continue;
            }
            let wrong = |e: String| fail(at.clone(), format!("{path} {e}"));
            let mut head =
                |keep| files.head(&path, MAX_MODEL_BYTES, keep).map_err(|e| fail(at.clone(), e.to_string()));
            let size = ModelFile::json_len(&head(20)?.0).map_err(wrong)?;
            json += size;
            if json > MAX_MODELS_JSON {
                return Err(wrong(format!(
                    "passes the {} MiB of model data a scene may read",
                    MAX_MODELS_JSON >> 20
                )));
            }
            let (bytes, total) = head(20 + size)?;
            let file = Arc::new(ModelFile::parse_head(&bytes, total, &mut left).map_err(wrong)?);
            read.insert(path, file.clone());
            self.models.insert(id, file);
        }
        Ok(())
    }
}
