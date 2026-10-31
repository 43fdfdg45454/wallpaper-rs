use super::*;

impl Run {
    pub fn shot(&mut self) -> Shot {
        self.shot_at((W, H))
    }

    pub fn shot_at(&mut self, size: (u32, u32)) -> Shot {
        self.shot_edited(size, |_| {})
    }

    pub fn shot_edited(&mut self, (w, h): (u32, u32), edit: impl FnOnce(&mut wprs_scene::Frame)) -> Shot {
        let mut frame = self.runtime.frame(self.view.view((w, h)), self.quality);
        edit(&mut frame);
        let image =
            self.r.render_scene(&mut self.view, &self.runtime, &frame, (w, h), self.quality, &[]).unwrap();
        let (mut target, mut pixels) = (self.r.create_target(w, h).unwrap(), Vec::new());
        let layer = Layer {
            image: Some(image),
            fit: Fit::Stretch,
            background: [0; 3],
            opacity: 1.0,
            uv: None,
            relief: None,
        };
        self.r.render_layers(&mut target, &[layer], &mut pixels).unwrap();
        Shot(RgbaImage::from_fn(w, h, |x, y| {
            let i = ((y * w + x) * 4) as usize;
            Rgba([pixels[i + 2], pixels[i + 1], pixels[i], pixels[i + 3]])
        }))
    }

    pub fn stats(&self) -> SceneStats {
        self.view.stats()
    }

    pub fn pointer(&mut self, at: [f32; 2]) {
        let still = PointerState {
            pos: Some(at),
            smooth: at,
            inside: true,
            presence: 1.0,
            ..PointerState::default()
        };
        self.runtime.set_pointer(still);
    }
}

impl Shot {
    pub fn sheet(shots: &[Shot]) -> Shot {
        let mut sheet = RgbaImage::new(W, H * shots.len() as u32);
        for (n, shot) in shots.iter().enumerate() {
            image::imageops::replace(&mut sheet, &shot.0, 0, i64::from(n as u32 * H));
        }
        Shot(sheet)
    }

    pub fn at(&self, x: u32, y: u32) -> [u8; 3] {
        let [r, g, b, _] = self.0.get_pixel(x, y).0;
        [r, g, b]
    }

    pub fn check(&self, points: &[((u32, u32), [u8; 3])]) {
        for &((x, y), expected) in points {
            let found = self.at(x, y);
            let near = found.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 3);
            assert!(near, "pixel {x},{y} is {found:?}, expected {expected:?}");
        }
    }

    pub fn mean(&self) -> [f32; 3] {
        let n = (W * H) as f32;
        [0, 1, 2].map(|c| self.0.pixels().map(|p| f32::from(p.0[c])).sum::<f32>() / n)
    }

    pub fn looks_like(&self, name: &str) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/effects/{name}.png"));
        if std::env::var_os("WPRS_BLESS").is_some() {
            return self.0.save(&path).unwrap();
        }
        let reference = image::open(&path).unwrap().into_rgba8();
        let near = |a: &Rgba<u8>, b: &Rgba<u8>| a.0.iter().zip(b.0).all(|(x, y)| x.abs_diff(y) <= 24);
        let off = self.0.pixels().zip(reference.pixels()).filter(|(a, b)| !near(a, b)).count();
        let allowed = 8 * (self.0.height() / H).max(1) as usize;
        assert!(self.0.dimensions() == reference.dimensions(), "{name}: the size changed");
        assert!(off <= allowed, "{name}: {off} pixels differ from tests/effects/{name}.png");
    }

    pub fn diff(&self, other: &Shot) -> f32 {
        let pairs = self.0.as_raw().iter().zip(other.0.as_raw());
        pairs.map(|(a, b)| f32::from(a.abs_diff(*b))).sum::<f32>() / self.0.as_raw().len() as f32
    }
}
