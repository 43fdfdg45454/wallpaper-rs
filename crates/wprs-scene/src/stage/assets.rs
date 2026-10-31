use super::{IMAGES, VIDEOS};
use crate::MAX_FRAMES;
use crate::validate::{Check, Issue};
use crate::value::Value;

impl Check<'_> {
    pub(crate) fn assets(&mut self) {
        for (id, a) in &self.scene.assets {
            let (p, ext) = (format!("/assets/{id}"), a.extension());
            let picture = IMAGES.contains(&ext.as_str());
            self.uses(a.sampled(), "texture_options");
            self.uses(a.collection(), "fonts2");
            self.uses(a.moves(), "animated_textures");
            let face = a.face.is_some() && !["ttc", "otc"].contains(&ext.as_str());
            let sheet = a.atlas.is_some() || !a.rects.is_empty() || a.animated;
            let set =
                |fields: [(&'static str, bool); 3]| fields.into_iter().find(|(_, on)| *on).map(|(n, _)| n);
            let sampled = set([
                ("filter", a.filter.is_some()),
                ("wrap", a.wrap.is_some()),
                ("mipmaps", a.mipmaps.is_some()),
            ]);
            let timed = set([
                ("frames", a.frames.is_some()),
                ("fps", a.fps.is_some()),
                ("durations", !a.durations.is_empty()),
            ]);
            let wrong = [
                (sampled.unwrap_or_default(), !picture),
                ("face", face),
                ("animated", a.animated && !["gif", "webp"].contains(&ext.as_str())),
                ("rects", !a.rects.is_empty() && (a.atlas.is_some() || !picture)),
                (timed.unwrap_or_default(), !sheet || !picture),
                ("durations", a.fps.is_some() && !a.durations.is_empty()),
            ];
            let wrong = wrong.into_iter().filter(|(name, _)| !name.is_empty());
            for (name, _) in wrong.filter(|(_, w)| *w) {
                self.issues.push(Issue::Type(format!("{p}/{name}")));
            }
            if a.media.is_some() {
                self.uses(true, "media_info");
                if !picture || a.moves() {
                    self.issues.push(Issue::Type(format!("{p}/media")));
                }
            }
            a.face.iter().for_each(|f| self.range(&format!("{p}/face"), *f as f32, 0.0, 255.0));
            self.count(&format!("{p}/rects"), a.rects.len(), MAX_FRAMES as usize);
            for (i, r) in a.rects.iter().enumerate() {
                self.range(&format!("{p}/rects/{i}/2"), r[2] as f32, 1.0, 16_384.0);
                self.range(&format!("{p}/rects/{i}/3"), r[3] as f32, 1.0, 16_384.0);
            }
            let cells = if a.animated { MAX_FRAMES } else { a.cells().min(MAX_FRAMES) };
            a.frames.iter().for_each(|n| self.range(&format!("{p}/frames"), *n as f32, 1.0, cells as f32));
            a.fps.iter().for_each(|f| self.range(&format!("{p}/fps"), *f, 0.1, 120.0));
            if !a.durations.is_empty() {
                let n = a.frames.unwrap_or(cells) as f32;
                self.range(&format!("{p}/durations"), a.durations.len() as f32, n, n);
                for (i, d) in a.durations.iter().enumerate() {
                    self.range(&format!("{p}/durations/{i}"), *d, 0.001, 60.0);
                }
            }
        }
    }

    pub(crate) fn moving(&mut self, named: &Value<String>) {
        let ids: Vec<&String> = match named {
            Value::Fixed(id) => vec![id],
            Value::Bound(b) => b.map.iter().flat_map(|m| m.values()).collect(),
        };
        let assets = ids.into_iter().filter_map(|id| self.scene.assets.get(id));
        let video = assets.into_iter().any(|a| VIDEOS.contains(&a.extension().as_str()));
        self.uses(video, "video_textures");
    }
}
