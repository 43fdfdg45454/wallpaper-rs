use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShakeSet {
    pub enabled: Option<bool>,
    pub amplitude: Option<f32>,
    pub roughness: Option<f32>,
    pub speed: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CameraSet {
    pub center: Option<[f32; 2]>,
    pub zoom: Option<f32>,
    pub rotation: Option<f32>,
    pub shake: Option<ShakeSet>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GlideSet {
    pub amount: Option<f32>,
    pub delay: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BloomSet {
    pub threshold: Option<f32>,
    pub strength: Option<f32>,
    pub radius: Option<f32>,
    pub tint: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StageSet {
    pub background: Option<[f32; 4]>,
    pub camera: Option<CameraSet>,
    pub parallax: Option<GlideSet>,
    pub bloom: Option<BloomSet>,
    pub ambient: Option<[f32; 4]>,
    pub sky: Option<[f32; 4]>,
}

fn within(v: Option<f32>, lo: f32, hi: f32) -> Option<f32> {
    v.filter(|v| v.is_finite()).map(|v| v.clamp(lo, hi))
}

fn color(c: Option<[f32; 4]>) -> Option<[f32; 4]> {
    c.filter(|c| c.iter().all(|v| v.is_finite())).map(|c| c.map(|v| v.clamp(0.0, 1.0)))
}

impl StageSet {
    pub fn bounded(self) -> StageSet {
        let shake = |s: ShakeSet| ShakeSet {
            enabled: s.enabled,
            amplitude: within(s.amplitude, 0.0, 1000.0),
            roughness: within(s.roughness, 0.0, 1.0),
            speed: within(s.speed, 0.0, 60.0),
        };
        let camera = |c: CameraSet| CameraSet {
            center: c
                .center
                .filter(|p| p.iter().all(|v| v.is_finite()))
                .map(|p| p.map(|v| v.clamp(-1e5, 1e5))),
            zoom: within(c.zoom, 0.1, 10.0),
            rotation: within(c.rotation, -360.0, 360.0),
            shake: c.shake.map(shake),
        };
        let glide =
            |g: GlideSet| GlideSet { amount: within(g.amount, 0.0, 4.0), delay: within(g.delay, 0.0, 2.0) };
        let bloom = |b: BloomSet| BloomSet {
            threshold: within(b.threshold, 0.0, 4.0),
            strength: within(b.strength, 0.0, 8.0),
            radius: within(b.radius, 0.0, 128.0),
            tint: color(b.tint),
        };
        StageSet {
            background: color(self.background),
            camera: self.camera.map(camera),
            parallax: self.parallax.map(glide),
            bloom: self.bloom.map(bloom),
            ambient: color(self.ambient),
            sky: color(self.sky),
        }
    }
}
