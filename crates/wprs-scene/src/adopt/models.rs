use super::Files;
use crate::gltf::{MAX_MODEL_BYTES, MAX_MODEL_ENTRIES, MAX_MODELS_JSON, ModelFile};
use crate::{Kind, Rigged, Scene, SceneError};
use std::collections::BTreeMap;
use std::sync::Arc;

type Read = (Arc<ModelFile>, Option<Arc<Rigged>>);

impl Scene {
    pub fn read_meshes(&mut self, mut files: impl Files) -> Result<(), SceneError> {
        let fail = |path: String, reason: String| SceneError::Invalid(crate::Issue::Effect { path, reason });
        self.read_models(&mut files, fail)
    }

    pub(super) fn read_models(
        &mut self,
        files: &mut impl Files,
        fail: impl Fn(String, String) -> SceneError + Copy,
    ) -> Result<(), SceneError> {
        let mut figures = BTreeMap::<String, bool>::new();
        for l in self.layers().chain(self.templates.values()) {
            let Some(id) = l.figure() else { continue };
            *figures.entry(id.to_owned()).or_default() |= matches!(l.kind, Kind::Mesh(_));
        }
        let (mut read, mut json, mut left, mut whole) =
            (BTreeMap::<String, Read>::new(), 0, MAX_MODEL_ENTRIES, 0);
        for (id, mesh) in figures {
            let Some(asset) = self.assets.get(&id).filter(|a| a.extension() == "glb") else { continue };
            let (path, at) = (asset.path.clone(), format!("/assets/{id}"));
            if let Some((file, rig)) = read.get(&path).filter(|(_, rig)| rig.is_some() || !mesh) {
                self.models.insert(id.clone(), file.clone());
                rig.iter().for_each(|r| _ = self.rigs.insert(id.clone(), r.clone()));
                continue;
            }
            let wrong = |e: String| fail(at.clone(), format!("{path} {e}"));
            let mut head =
                |keep| files.head(&path, MAX_MODEL_BYTES, keep).map_err(|e| fail(at.clone(), e.to_string()));
            let (bytes, total) = match mesh {
                true => {
                    let bytes =
                        files.read(&path, MAX_MODEL_BYTES).map_err(|e| fail(at.clone(), e.to_string()))?;
                    whole += bytes.len();
                    if whole > crate::MAX_RIG_BYTES {
                        return Err(wrong(format!(
                            "passes the {} MiB of meshes a scene may read",
                            crate::MAX_RIG_BYTES >> 20
                        )));
                    }
                    let total = bytes.len() as u64;
                    (bytes, total)
                }
                false => {
                    let size = ModelFile::json_len(&head(20)?.0).map_err(wrong)?;
                    head(20 + size)?
                }
            };
            json += ModelFile::json_len(&bytes).map_err(wrong)?;
            if json > MAX_MODELS_JSON {
                return Err(wrong(format!(
                    "passes the {} MiB of model data a scene may read",
                    MAX_MODELS_JSON >> 20
                )));
            }
            let file = Arc::new(ModelFile::parse_head(&bytes, total, &mut left).map_err(wrong)?);
            let rig = mesh.then(|| Rigged::decode(&bytes).map(Arc::new)).transpose().map_err(wrong)?;
            read.insert(path, (file.clone(), rig.clone()));
            self.models.insert(id.clone(), file);
            if let Some(rig) = rig {
                self.rigs.insert(id, rig);
            }
        }
        Ok(())
    }
}
