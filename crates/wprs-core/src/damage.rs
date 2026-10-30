use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

const KEPT: usize = 8;
static SERIAL: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Region {
    pub fn whole((width, height): (u32, u32)) -> Region {
        Region { x: 0, y: 0, width, height }
    }

    pub fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    pub fn union(self, o: Region) -> Region {
        match (self.is_empty(), o.is_empty()) {
            (true, _) => o,
            (_, true) => self,
            _ => {
                let (x, y) = (self.x.min(o.x), self.y.min(o.y));
                let right = (self.x + self.width).max(o.x + o.width);
                let bottom = (self.y + self.height).max(o.y + o.height);
                Region { x, y, width: right - x, height: bottom - y }
            }
        }
    }

    pub fn within(self, (width, height): (u32, u32)) -> Region {
        let (x, y) = (self.x.min(width), self.y.min(height));
        let (right, bottom) = ((self.x + self.width).min(width), (self.y + self.height).min(height));
        Region { x, y, width: right - x, height: bottom - y }
    }

    pub fn scaled(self, from: (u32, u32), to: (u32, u32)) -> Region {
        if self.is_empty() || from == to {
            return self.within(to);
        }
        let axis = |start: u32, len: u32, from: u32, to: u32| {
            let k = f64::from(to) / f64::from(from.max(1));
            let lo = (f64::from(start.saturating_sub(1)) * k).floor() as u32;
            let hi = (f64::from(start + len + 1) * k).ceil() as u32;
            (lo, hi.max(lo))
        };
        let (x0, x1) = axis(self.x, self.width, from.0, to.0);
        let (y0, y1) = axis(self.y, self.height, from.1, to.1);
        Region { x: x0, y: y0, width: x1 - x0, height: y1 - y0 }.within(to)
    }

    pub fn covers(self, size: (u32, u32)) -> bool {
        self.within(size) == Region::whole(size)
    }
}

pub fn join(a: Option<Region>, b: Option<Region>) -> Option<Region> {
    Some(a?.union(b?))
}

#[derive(Debug)]
pub struct Ages {
    kept: VecDeque<(u64, Option<Region>)>,
    id: u64,
}

impl Default for Ages {
    fn default() -> Ages {
        Ages { kept: VecDeque::new(), id: SERIAL.fetch_add(1, Relaxed) }
    }
}

impl Ages {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn push(&mut self, damage: Option<Region>) -> u64 {
        let serial = SERIAL.fetch_add(1, Relaxed);
        self.kept.reserve_exact(KEPT.saturating_sub(self.kept.len()));
        if self.kept.len() == KEPT {
            self.kept.pop_front();
        }
        self.kept.push_back((serial, damage));
        serial
    }

    pub fn serial(&self) -> u64 {
        self.kept.back().map_or(0, |k| k.0)
    }

    pub fn since(&self, serial: u64) -> Option<Region> {
        let at = self.kept.iter().position(|k| k.0 == serial)?;
        self.kept
            .iter()
            .skip(at + 1)
            .try_fold(Region::default(), |found, (_, damage)| Some(found.union((*damage)?)))
    }

    pub fn last(&self) -> Option<Region> {
        self.kept.back().and_then(|k| k.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: (u32, u32) = (100, 50);

    fn at(x: u32, y: u32, width: u32, height: u32) -> Region {
        Region { x, y, width, height }
    }

    #[test]
    fn regions_join_clip_and_scale_with_a_margin_for_filtering() {
        assert_eq!(at(10, 10, 5, 5).union(at(30, 2, 10, 4)), at(10, 2, 30, 13));
        assert_eq!(Region::default().union(at(1, 2, 3, 4)), at(1, 2, 3, 4));
        assert_eq!(at(90, 40, 30, 30).within(SIZE), at(90, 40, 10, 10));
        assert_eq!(at(10, 10, 10, 10).scaled((50, 25), SIZE), at(18, 18, 24, 24));
        assert_eq!(at(0, 0, 50, 25).scaled((50, 25), SIZE), Region::whole(SIZE));
        assert!(Region::whole(SIZE).covers(SIZE) && !at(0, 0, 99, 50).covers(SIZE));
        assert_eq!(join(Some(at(0, 0, 1, 1)), None), None);
    }

    #[test]
    fn a_buffer_gets_the_damage_of_every_frame_it_missed_and_old_ones_get_everything() {
        let mut ages = Ages::default();
        let first = ages.push(None);
        let second = ages.push(Some(at(10, 10, 5, 5)));
        ages.push(Some(at(40, 20, 5, 5)));
        assert_eq!(ages.since(ages.serial()), Some(Region::default()));
        assert_eq!(ages.since(second), Some(at(40, 20, 5, 5)));
        assert_eq!(ages.since(first), Some(at(10, 10, 35, 15)));
        assert_eq!(ages.since(0), None, "a buffer that was never drawn needs everything");
        ages.push(None);
        assert_eq!(ages.since(first), None, "a full frame since reaches every older buffer");
        (0..KEPT).for_each(|_| _ = ages.push(Some(at(0, 0, 1, 1))));
        assert_eq!(ages.since(second), None, "a buffer older than the history needs everything");
        let mut other = Ages::default();
        other.push(Some(at(0, 0, 1, 1)));
        assert_eq!(other.since(ages.serial()), None, "serials of another output never match");
        assert_ne!(other.id(), ages.id());
    }
}
