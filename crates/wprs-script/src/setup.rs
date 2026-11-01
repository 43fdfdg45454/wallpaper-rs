use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::collections::BTreeMap;
use wprs_scene::{Layer, MAX_SCRIPT_BYTES, Scene, ScriptApi, Site, sites};

pub type Source = (String, String);

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Setup {
    pub sources: Vec<Source>,
    pub seed: u64,
    pub locale: String,
    pub labels: Json,
    pub canvas: [u32; 2],
    pub mobile: bool,
    pub params: BTreeMap<String, BTreeMap<String, Json>>,
    pub uses: BTreeMap<String, Vec<ScriptApi>>,
    pub first: Vec<String>,
    pub checked: bool,
    pub sites: Vec<Site>,
    pub layers: BTreeMap<String, Json>,
    pub templates: BTreeMap<String, Json>,
    pub top: Vec<String>,
    pub stage: Json,
    pub pools: BTreeMap<String, Json>,
    pub storage: Json,
    pub folders: Vec<String>,
}

fn info(layer: &Layer, scene: &Scene) -> Json {
    let effects: Vec<Json> = (layer.effects.iter())
        .map(|e| {
            let params = scene.shader(&e.effect).into_iter().flat_map(|d| &d.params);
            let params: serde_json::Map<String, Json> = params
                .map(|p| (p.id.clone(), json!({ "kind": p.kind, "min": p.min, "max": p.max })))
                .collect();
            json!({ "effect": e.effect, "name": e.name, "params": params })
        })
        .collect();
    let kind =
        serde_json::to_value(&layer.kind).ok().and_then(|k| k.get("type").cloned()).unwrap_or_default();
    let mut initial = serde_json::to_value(layer).unwrap_or_default();
    if let Some(group) = initial.get_mut("layers") {
        *group = Json::Array(layer.children().iter().map(|c| json!(c.id)).collect());
    }
    json!({ "type": kind, "effects": effects, "initial": initial, "rig": rig(layer, scene) })
}

fn rig(layer: &Layer, scene: &Scene) -> Json {
    let wprs_scene::Kind::Mesh(m) = &layer.kind else { return Json::Null };
    let Some(r) = scene.rigs.get(&m.mesh) else { return Json::Null };
    let bones: Vec<Json> = (r.joints.iter())
        .map(|j| {
            let ([x, y, _], q, s) = (j.rest.t, j.rest.r, j.rest.s);
            let turn = -2.0 * q[2].atan2(q[3]).to_degrees();
            json!({ "name": j.name, "parent": j.parent, "position": [x, -y], "rotation": turn, "scale": [s[0], s[1]] })
        })
        .collect();
    let size = m.size.unwrap_or(scene.canvas.size.map(|v| v as f32));
    json!({ "size": size, "bones": bones, "shapes": r.shapes, "hit": m.rig.hit })
}

fn nested(prefix: &str, layer: &Layer, scene: &Scene, out: &mut BTreeMap<String, Json>) {
    out.insert(prefix.to_owned(), info(layer, scene));
    layer.children().iter().for_each(|c| nested(&format!("{prefix}.{}", c.id), c, scene, out));
}

fn stage(scene: &Scene) -> Json {
    let c = &scene.canvas;
    json!({ "background": c.background, "camera": c.camera, "parallax": c.parallax, "bloom": c.bloom })
}

impl Setup {
    pub fn new(scene: &Scene, sources: &[Source], seed: u64, (locale, labels): (&str, &Json)) -> Setup {
        let params = scene.scripts.iter().map(|s| {
            let defaults = s
                .params
                .iter()
                .map(|(id, p)| (id.clone(), serde_json::to_value(&p.default).unwrap_or_default()));
            (s.path.clone(), defaults.collect())
        });
        let mut templates = BTreeMap::new();
        scene.templates.iter().for_each(|(id, t)| nested(id, t, scene, &mut templates));
        Setup {
            sources: sources.to_vec(),
            seed,
            locale: locale.to_owned(),
            labels: if labels.is_object() { labels.clone() } else { json!({}) },
            canvas: scene.canvas.size,
            mobile: false,
            params: params.collect(),
            uses: scene.scripts.iter().map(|s| (s.path.clone(), s.uses.clone())).collect(),
            first: scene.first_interface(),
            checked: true,
            sites: sites(scene),
            layers: scene.layers().map(|l| (l.id.clone(), info(l, scene))).collect(),
            templates,
            top: scene.layers.iter().map(|l| l.id.clone()).collect(),
            stage: stage(scene),
            pools: scene
                .pools
                .iter()
                .map(|(id, p)| (id.clone(), serde_json::to_value(p).unwrap_or_default()))
                .collect(),
            storage: Json::Null,
            folders: Vec::new(),
        }
    }

    pub fn plain(sources: &[Source]) -> Setup {
        let (sources, seed, labels, canvas) = (sources.to_vec(), 1, json!({}), [1920, 1080]);
        Setup { sources, seed, labels, canvas, ..Setup::default() }
    }

    pub fn bytes(&self) -> usize {
        self.sources.iter().map(|(_, s)| s.len()).sum()
    }

    pub fn fits(&self) -> Result<(), String> {
        match self.sources.iter().find(|(_, s)| s.len() > MAX_SCRIPT_BYTES) {
            Some((path, _)) => Err(format!("{path} is larger than 1 MiB")),
            None if self.bytes() > MAX_SCRIPT_BYTES => {
                Err("the scripts take more than 1 MiB together".into())
            }
            None => Ok(()),
        }
    }
}
