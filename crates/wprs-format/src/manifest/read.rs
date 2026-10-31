use super::{Header, Manifest, PREFIX, newer_names};
use crate::{Error, Result, features};
use std::collections::BTreeSet;
use wprs_core::message::said;
use wprs_scene::Control;

impl Manifest {
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        Self::read(bytes, true)
    }

    pub(crate) fn read(bytes: &[u8], strict: bool) -> Result<Self> {
        let header: Header = serde_json::from_slice(bytes)?;
        let major = header.schema.strip_prefix(PREFIX).and_then(|n| n.parse::<u32>().ok());
        if let Some(major) = major.filter(|m| *m > 2) {
            return Err(Error::Newer(
                said("format.manifest_version", "manifest version %1", &[&major]).into(),
            ));
        }
        features::supported(header.requires.engine.as_deref(), &header.requires.features)?;
        let mut manifest: Manifest = serde_json::from_slice(bytes)?;
        if manifest.v2() {
            return Ok(manifest);
        }
        let newer = header.properties.iter().find(|(_, p)| !wprs_scene::legacy(p)).filter(|_| strict);
        if let Some((id, _)) = newer {
            let reason = said(
                "format.newer_properties",
                "the newer kinds and fields of properties need wallpaper-rs.wpak/2",
                &[],
            );
            return Err(crate::refuse(&format!("properties.{id}"), reason));
        }
        for property in manifest.properties.values_mut() {
            if let Control::File { accept, .. } = &mut property.control {
                accept.retain(|a| a == "image");
            }
        }
        Ok(manifest)
    }

    pub(crate) fn newer_names(raw: &[u8], scene: Option<&str>, features: &BTreeSet<String>) -> Vec<String> {
        let parsed = |text: &str| serde_json::from_str(text).is_ok_and(|v| wprs_scene::newer_scene_keys(&v));
        let scene = || scene.is_some_and(parsed);
        serde_json::from_slice(raw).map_or(Vec::new(), |m| newer_names(&m, scene, features))
    }
}
