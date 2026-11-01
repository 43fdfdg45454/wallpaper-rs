use crate::effects::Descriptor;
use crate::value::Param;
use crate::{MAX_PARAMS, Quality};
use std::borrow::Cow;

impl Descriptor {
    pub fn tuned(&self, quality: Quality, id: &str) -> Option<&Param> {
        self.quality.range(quality..).find_map(|(_, t)| t.params.get(id))
    }

    pub fn setting<'a>(
        &'a self,
        quality: Quality,
        id: &str,
        set: Option<Cow<'a, Param>>,
    ) -> Option<Cow<'a, Param>> {
        let Some(cap) = self.tuned(quality, id) else { return set };
        let value = set.or_else(|| self.param(id).map(|i| Cow::Borrowed(&self.params[i].default)));
        Some(match (cap, value.as_deref()) {
            (Param::Number(c), Some(Param::Number(v))) => Cow::Owned(Param::Number(v.min(*c))),
            _ => Cow::Borrowed(cap),
        })
    }

    pub fn frozen(&self, quality: Quality) -> bool {
        self.quality.range(quality..).any(|(_, t)| t.freeze)
    }

    pub fn settle(&self, slots: &[[f32; 4]; MAX_PARAMS]) -> f32 {
        let Some(s) = &self.state else { return 0.0 };
        let p = s.fade.as_deref().and_then(|f| self.param(f)).map_or(0.0, |i| slots[i][0]);
        let fading = if p > 0.0 { (1.0f32 / 512.0).ln() / p.min(0.9999).ln() / 60.0 } else { 0.0 };
        s.settle.max(fading.min(60.0))
    }

    pub fn scale_at(&self, quality: Quality) -> f32 {
        self.quality.range(quality..).find_map(|(_, t)| t.scale).unwrap_or(1.0)
    }
}
