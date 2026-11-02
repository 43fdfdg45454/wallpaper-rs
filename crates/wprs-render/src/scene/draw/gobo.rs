use super::super::Src;
use super::super::paint::Painter;
use std::hash::{DefaultHasher, Hash, Hasher};
use wprs_scene::{MAX_PROJECTIONS, Picture};

impl Painter<'_, '_, '_> {
    fn drawn_at(&self, id: &str) -> Option<usize> {
        self.frame.draws.iter().position(|d| d.layer.id == id)
    }

    pub(crate) fn gobos(&mut self) -> [Src; MAX_PROJECTIONS] {
        if let Some(found) = self.pool.glow.gobos {
            return found;
        }
        let mut out = [Src::White; MAX_PROJECTIONS];
        let frame = self.frame;
        for (k, slot) in out.iter_mut().enumerate().take(self.pool.glow.pictures()) {
            let picture = frame.lamps.get(self.pool.glow.slots[k]).and_then(|l| l.picture.as_ref());
            let found = match picture {
                Some(Picture::Asset(id)) => self.picture(id).map(|g| self.ext(g)),
                Some(Picture::Layer(id)) => self.drawn_at(id).and_then(|j| self.shared(j)).map(|(s, _)| s),
                None => None,
            };
            *slot = found.unwrap_or(Src::White);
        }
        self.pool.glow.gobos = Some(out);
        out
    }

    pub(crate) fn watch_gobos(&mut self) {
        let (mut h, mut live) = (DefaultHasher::new(), false);
        let frame = self.frame;
        for k in 0..self.pool.glow.pictures() {
            match frame.lamps.get(self.pool.glow.slots[k]).and_then(|l| l.picture.as_ref()) {
                Some(Picture::Asset(id)) => live |= self.videos.iter().any(|(n, _)| n == id),
                Some(Picture::Layer(id)) => {
                    let Some(j) = self.drawn_at(id) else { continue };
                    let (key, moving) = self.upstream(j);
                    (live, _) = (live || moving, key.hash(&mut h));
                }
                None => {}
            }
        }
        (self.pool.glow.live, self.pool.glow.seen) = (live, h.finish());
    }
}
