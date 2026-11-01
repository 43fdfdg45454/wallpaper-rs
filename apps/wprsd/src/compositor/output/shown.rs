use super::super::scene::SceneState;
use super::super::slot::PlayerSlot;
use super::super::surface::Screen;
use super::super::{Content, Reel, Spec};
use super::Raised;
use std::collections::HashMap;
use std::time::Duration;
use wprs_core::{Fit, UvRect};
use wprs_render::{GpuImage, Layer, Relief, Target};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Tone {
    pub flip: [bool; 2],
    pub grade: Option<wprs_core::Grade>,
}

pub(in crate::compositor) struct Shown {
    pub(in crate::compositor) content: Content,
    pub(in crate::compositor) fit: Fit,
    pub(in crate::compositor) background: [u8; 3],
    pub(in crate::compositor) image: Option<GpuImage>,
    pub(in crate::compositor) transition: Option<Duration>,
    pub(in crate::compositor) uv: Option<UvRect>,
    pub(in crate::compositor) relief: Option<Raised>,
    pub(in crate::compositor) scene: Option<SceneState>,
    pub(in crate::compositor) tone: Tone,
}

impl Shown {
    pub(in crate::compositor) fn matches(&self, spec: &Spec) -> bool {
        self.content.same_look(&spec.content) && self.fit == spec.fit && self.background == spec.background
    }

    pub(in crate::compositor) fn layer<'a>(
        &'a self,
        players: &'a HashMap<Reel, PlayerSlot>,
        opacity: f32,
        moved: Option<([f32; 2], u32)>,
        size: (u32, u32),
    ) -> Layer<'a> {
        let (image, fit) = match &self.content {
            Content::Color => (None, self.fit),
            Content::Image(_) => (self.image.as_ref(), self.fit),
            Content::Video(m) => (players.get(&m.reel).and_then(|p| p.texture.as_ref()), self.fit),
            Content::Scene(_) => (self.scene.as_ref().and_then(SceneState::image), Fit::Stretch),
            Content::Web(_) => (self.image.as_ref(), Fit::Stretch),
        };
        let framed = match (&self.content, image) {
            (Content::Video(m), Some(i)) if m.zoom != 1.0 || m.pan != [0.0; 2] => {
                Some(UvRect::compute(fit, i.size(), size).framed(m.zoom, m.pan))
            }
            _ => self.uv,
        };
        let aspect = image.map_or(1.0, |i| i.size().0 as f32 / i.size().1.max(1) as f32);
        let relief = self.relief.as_ref().zip(moved).map(|((depth, p), ([x, y], steps))| {
            let reach = x.hypot(y).max(1.0);
            let shift = [p.strength * x / reach, p.strength * y * aspect / reach];
            Relief { depth, shift, focus: p.focus, steps: p.steps.min(steps), overscan: p.overscan }
        });
        let (flip, grade) = (self.tone.flip, self.tone.grade);
        Layer { image, fit, background: self.background, opacity, uv: framed, relief, flip, grade }
    }
}

pub(in crate::compositor) enum Sink {
    Stream(Option<Target>),
    Surface(Screen),
}
