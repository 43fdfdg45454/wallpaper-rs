use super::*;

impl Fx {
    pub fn new(effect: &str, params: Value) -> Fx {
        Fx::chain(json!([{ "effect": effect, "params": params }]))
    }

    pub fn chain(effects: Value) -> Fx {
        let (assets, files, own) = Default::default();
        Fx {
            effects,
            input: ramp(),
            assets,
            files,
            adjust: false,
            quality: Quality::Q3,
            own,
            unchecked: Vec::new(),
            background: "#000000",
            more: Vec::new(),
            space: "linear",
        }
    }

    pub fn input(self, input: RgbaImage) -> Fx {
        Fx { input, ..self }
    }

    pub fn adjust(self) -> Fx {
        Fx { adjust: true, ..self }
    }

    pub fn quality(self, quality: Quality) -> Fx {
        Fx { quality, ..self }
    }

    pub fn image(mut self, id: &str, image: RgbaImage, linear: bool) -> Fx {
        self.assets.insert(id.into(), json!({ "path": format!("{id}.png"), "linear": linear }));
        self.files.images.insert(id.into(), Pixels::new(image));
        self
    }

    pub fn background(self, background: &'static str) -> Fx {
        Fx { background, ..self }
    }

    pub fn layer(mut self, layer: Value) -> Fx {
        self.more.push(layer);
        self
    }

    pub fn space(self, space: &'static str) -> Fx {
        Fx { space, ..self }
    }

    pub fn own(mut self, json: Value, wgsl: &str) -> Fx {
        self.own.push((json["id"].as_str().unwrap_or_default().into(), json, wgsl.into()));
        self
    }

    pub fn unchecked(mut self, id: &str, wgsl: &str) -> Fx {
        self.unchecked.push((id.into(), wgsl.into()));
        self
    }

    pub fn lut(mut self, id: &str, text: &str) -> Fx {
        self.assets.insert(id.into(), json!({ "path": format!("{id}.cube") }));
        self.files.luts.insert(id.into(), Arc::new(Lut::parse(text.as_bytes()).unwrap()));
        self
    }

    pub fn run(self) -> Option<Run> {
        let r = super::super::renderer()?;
        let (effects, adjustment) = match self.adjust {
            true => (json!([]), Some(json!({ "id": "fx", "type": "adjustment", "effects": self.effects }))),
            false => (self.effects, None),
        };
        let image =
            json!({ "id": "src", "type": "image", "image": "src", "size": [W, H], "effects": effects });
        let layers: Vec<Value> = std::iter::once(image).chain(self.more).chain(adjustment).collect();
        let mut assets = self.assets;
        assets.insert("src".into(), json!({ "path": "src.png" }));
        let canvas = json!({ "size": [W, H], "background": self.background, "blend_space": self.space });
        let own: Map<String, Value> = self
            .own
            .iter()
            .map(|(id, ..)| {
                (
                    id.clone(),
                    json!({ "descriptor": format!("fx/{id}.json"), "wgsl": format!("fx/{id}.wgsl") }),
                )
            })
            .collect();
        let text =
            json!({ "schema": SCHEMA, "canvas": canvas, "assets": assets, "layers": layers, "effects": own });
        let mut scene = Scene::from_json(&text.to_string()).unwrap_or_else(|e| panic!("{e}"));
        let files = |path: &str, _: usize| {
            let (id, ext) = path.trim_start_matches("fx/").split_once('.').unwrap_or_default();
            let (_, json, wgsl) = self.own.iter().find(|(i, ..)| i == id).ok_or("missing")?;
            Ok::<_, &str>(if ext == "json" { json.to_string() } else { wgsl.clone() }.into_bytes())
        };
        scene.adopt(&BTreeMap::new(), files).unwrap_or_else(|e| panic!("{e}"));
        for (id, wgsl) in &self.unchecked {
            let d = scene.loaded.get_mut(id).expect("a package effect");
            let key = d.key ^ wgsl.bytes().fold(0u64, |h, b| h.rotate_left(5) ^ u64::from(b)) << 1;
            *d = Arc::new(Descriptor { wgsl: wgsl.clone(), key, ..(**d).clone() });
        }
        let mut files = self.files;
        files.images.insert("src".into(), Pixels::new(self.input));
        let view = r.scene_view(&scene, &files, &Values::new()).unwrap();
        Some(Run { r, runtime: Runtime::new(Arc::new(scene), Values::new()), view, quality: self.quality })
    }

    pub fn shot(self) -> Option<Shot> {
        Some(self.run()?.shot())
    }
}
