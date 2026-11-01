use crate::apply::target_output;
use crate::compositor::{Movie, Reel};
use crate::daemon::Daemon;
use crate::error::{Result, invalid, not_found};
use crate::keyring::Keyring;
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use wprs_core::model::{MODEL, SceneOption, SceneProperties};
use wprs_scene::{Control, Preset, Property, Values};
use wprs_video::Cut;

pub type Stored = BTreeMap<String, toml::Value>;

pub struct Props {
    pub path: PathBuf,
    pub id: String,
    pub version: String,
    pub title: String,
    pub kind: &'static str,
    pub languages: Vec<String>,
    pub properties: BTreeMap<String, Property>,
    pub presets: Vec<Preset>,
    pub authored: Authored,
}

#[derive(Debug, Clone, Default)]
pub struct Authored {
    pub playback: wprs_format::Playback,
    pub accent: Option<wprs_format::Accent>,
}

impl Authored {
    pub fn of(m: &wprs_format::Manifest) -> Authored {
        let accent = m.display.as_ref().and_then(|d| d.accent.clone());
        Authored { playback: m.playback.clone().unwrap_or_default(), accent }
    }
}

pub fn stored(value: &Json) -> Option<toml::Value> {
    (!value.is_null()).then(|| toml::Value::try_from(value).ok()).flatten()
}

fn layers(stored: &[&Stored]) -> Vec<Values> {
    let pairs = |l: &&Stored| {
        l.iter().filter_map(|(k, v)| Some((k.clone(), serde_json::to_value(v).ok()?))).collect()
    };
    stored.iter().map(pairs).collect()
}

impl Props {
    pub fn values(&self, stored: &[&Stored]) -> Values {
        wprs_scene::effective(&self.properties, &layers(stored).iter().collect::<Vec<_>>())
    }

    pub fn stale(&self, stored: &[&Stored]) -> Vec<String> {
        wprs_scene::stale(&self.properties, &layers(stored).iter().collect::<Vec<_>>())
    }

    pub fn movie(&self, path: PathBuf, values: &Values) -> Movie {
        let num = |id: &str| values.get(id).and_then(Json::as_f64).unwrap_or(0.0);
        let (start, end) = ((num("start") * 1000.0) as u32, (num("end") * 1000.0) as u32);
        let end = if end > start + 100 { end } else { 0 };
        let cut = Cut { speed: (num("speed") * 100.0).round() as u32, start, end };
        let play = &self.authored.playback;
        let (reel, volume) = (Reel { path, cut, once: play.once() }, num("volume").round() as u32);
        let (zoom, pan, max_fps) = (num("zoom") as f32, [num("x") as f32, num("y") as f32], play.frame_cap());
        Movie { reel, volume, zoom, pan, max_fps }
    }
}

impl Daemon {
    pub fn video_props(&self, path: &Path) -> anyhow::Result<Arc<Props>> {
        let m = crate::decode::open(path)?.manifest.localized();
        anyhow::ensure!(m.kind == wprs_format::Kind::Video, "{} is not a video package", path.display());
        let languages = m.languages(&wprs_core::locale::languages());
        let item = |lib: &wprs_library::Library| lib.item(&lib.resolve_item(&path.to_string_lossy())?);
        let length =
            self.store.read(item).ok().and_then(|i| i.meta.duration_ms).map(|ms| f64::from(ms) / 1000.0);
        let properties = m.video_properties(&crate::words::video(), length);
        let (id, version, title) = (m.id.clone(), m.version.clone(), m.title.pick(&languages));
        let mut authored = Authored::of(&m);
        authored.accent = authored.accent.and_then(|a| a.settled(&m.properties));
        let presets = m.presets;
        Ok(Arc::new(Props {
            path: path.into(),
            id,
            version,
            title,
            kind: "video",
            languages,
            properties,
            presets,
            authored,
        }))
    }

    pub fn package_props(&self, path: &Path) -> anyhow::Result<Arc<Props>> {
        match crate::decode::open(path)?.manifest.kind {
            wprs_format::Kind::Scene => Ok(self.scene_pack(path)?.props.clone()),
            wprs_format::Kind::Web => Ok(self.web_pack(path)?.props.clone()),
            _ => self.video_props(path),
        }
    }

    pub fn props_on(&self, output: &str) -> Result<(String, Arc<Props>)> {
        let output = target_output(output)?;
        let (shown, path) = {
            let st = self.state();
            let info = st.outputs.get(&output).filter(|_| !output.is_empty());
            let shown =
                info.and_then(|i| i.scene.as_ref().map(|s| s.props.clone()).or_else(|| i.video.clone()));
            let path = if output.is_empty() {
                st.config.default.image.clone()
            } else {
                st.config.resolve(&output).image
            };
            (shown, path)
        };
        let section = if output.is_empty() { "the default section" } else { &output };
        let missing = || not_found(format!("{section} shows no package with settings"));
        let package = |p: &PathBuf| wprs_library::Kind::detect(p) == Some(wprs_library::Kind::Package);
        let props = match (shown, path) {
            (Some(props), _) => props,
            (None, Some(path)) if package(&path) => self.package_props(&path).map_err(|_| missing())?,
            _ => return Err(missing()),
        };
        Ok((output, props))
    }

    pub fn scene_properties(&self, output: &str) -> Result<SceneProperties> {
        let (output, props) = self.props_on(output)?;
        let (values, warnings, shared) = {
            let st = self.state();
            let stored = st.config.scene_values(&output, &props.id);
            let own = st.config.outputs.get(&output).is_some_and(|o| o.scenes.contains_key(&props.id));
            (props.values(&stored), props.stale(&stored), !output.is_empty() && !own)
        };
        let (fields, preset_labels) =
            wprs_scene::form(&props.properties, &props.presets, &values, &props.languages);
        let mine = self.user_presets(&props.id)?.into_iter();
        let user_presets =
            mine.map(|p| SceneOption { value: json!(p.uid), label: p.name, icon: String::new() }).collect();
        let properties = props.properties.iter().map(|(k, p)| (k.clone(), json!(p))).collect();
        let presets = props.presets.iter().map(|p| json!(p)).collect();
        let (package, title, kind) = (props.id.clone(), props.title.clone(), props.kind.into());
        let serial = self.changes.serial();
        Ok(SceneProperties {
            model: MODEL,
            serial,
            output,
            package,
            title,
            properties,
            values,
            presets,
            fields,
            preset_labels,
            warnings,
            user_presets,
            shared,
            kind,
        })
    }

    fn property_keyring(&self) -> Result<Keyring> {
        Ok(self.keyring()?.properties())
    }

    pub async fn store_secret(&self, key: &str, value: &str) -> Result<()> {
        let text: String =
            serde_json::from_str(value).map_err(|_| invalid("a secret text is a JSON string"))?;
        if text.chars().count() > 1024 || text.chars().any(char::is_control) {
            return Err(invalid("a secret text has at most 1024 characters and no control characters"));
        }
        match text.is_empty() {
            true => self.property_keyring()?.delete(key).await?,
            false => self.property_keyring()?.store(key, &text).await?,
        }
        self.changes.changed("state", Vec::new());
        Ok(())
    }

    pub async fn fill_secrets(&self, reply: &mut SceneProperties) {
        let Ok(keyring) = self.property_keyring() else { return };
        for field in reply.fields.iter_mut().filter(|f| f.secret) {
            field.filled = keyring.has(&format!("{}/{}", reply.package, field.key)).await.unwrap_or(false);
        }
    }

    pub fn secret_key(&self, output: &str, id: &str) -> Result<Option<String>> {
        let (_, props) = self.props_on(output)?;
        let secret =
            matches!(props.properties.get(id).map(|p| &p.control), Some(Control::Text { secret: true, .. }));
        Ok(secret.then(|| format!("{}/{id}", props.id)))
    }
}
