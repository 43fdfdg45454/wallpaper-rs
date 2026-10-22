use super::Content;
use super::iio::Sensor;
use super::state::State;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use wprs_core::edit::Motion as Source;
use wprs_core::motion::{Spring, orbit};

const IDLE: Duration = Duration::from_secs(3);
const SUBPIXEL: f32 = 0.25;

type Key = (PathBuf, Source, u32);
type Live = (String, Key, [f32; 2], f32);

#[derive(Default)]
pub(super) struct Motion {
    pub(super) calm: bool,
    pub(super) software: bool,
    pointers: BTreeMap<Key, ([f32; 2], Instant)>,
    started: Option<Instant>,
    springs: BTreeMap<Key, Spring>,
    goals: BTreeMap<Key, [f32; 2]>,
    last: Option<Instant>,
    sensor: Sensor,
}

impl Motion {
    pub(super) fn new(software: bool) -> Self {
        Self { software, ..Self::default() }
    }

    fn goal(&mut self, key: &Key, now: Instant) -> [f32; 2] {
        let started = *self.started.get_or_insert(now);
        match key.1 {
            Source::Auto => orbit(now.saturating_duration_since(started).as_secs_f32()),
            Source::Pointer => {
                self.pointers.get(key).filter(|(_, at)| now < *at + IDLE).map_or([0.0; 2], |p| p.0)
            }
            Source::Accelerometer => self.sensor.goal(now, None),
        }
    }

    pub(super) fn due(&self, max_fps: u32) -> Option<Instant> {
        let last = self.last?;
        let frame = Duration::from_secs(1) / max_fps.max(1);
        let restless = |(key, spring): (&Key, &Spring)| match key.1 {
            Source::Auto => true,
            Source::Accelerometer if self.sensor.present() => true,
            _ => !spring.settled(self.goals.get(key).copied().unwrap_or_default()),
        };
        let ticking = self.springs.iter().any(restless).then_some(last + frame);
        let idle = self.pointers.values().filter(|(p, at)| *p != [0.0; 2] && *at + IDLE > last);
        let rescan =
            self.sensor.rescan().filter(|_| self.springs.keys().any(|k| k.1 == Source::Accelerometer));
        ticking.into_iter().chain(idle.map(|(_, at)| *at + IDLE)).chain(rescan).min()
    }

    fn step(
        &mut self,
        live: &[Live],
        pointer: impl Fn(&[&str]) -> Option<[f32; 2]>,
        now: Instant,
    ) -> Vec<(String, [f32; 2])> {
        let dt = self.last.map_or(0.0, |t| now.saturating_duration_since(t).as_secs_f32().min(0.1));
        self.last = (!live.is_empty()).then_some(now);
        let keys: BTreeSet<&Key> = live.iter().map(|l| &l.1).collect();
        self.springs.retain(|key, _| keys.contains(key));
        self.pointers.retain(|key, _| keys.contains(key));
        for key in keys.iter().filter(|k| k.1 == Source::Pointer) {
            let names: Vec<&str> = live.iter().filter(|l| l.1 == **key).map(|l| l.0.as_str()).collect();
            let at = pointer(&names).unwrap_or_default();
            if self.pointers.get(*key).is_none_or(|(p, _)| *p != at) {
                self.pointers.insert((*key).clone(), (at, now));
            }
        }
        for (_, key, offset, _) in live {
            self.springs.entry(key.clone()).or_insert(Spring { at: *offset, speed: [0.0; 2] });
        }
        self.goals = keys.iter().map(|key| ((*key).clone(), self.goal(key, now))).collect();
        for (key, spring) in &mut self.springs {
            spring.step(self.goals.get(key).copied().unwrap_or_default(), key.2 as f32 / 1000.0, dt);
        }
        let at = |key: &Key| self.springs.get(key).map_or([0.0; 2], |s| s.at);
        let visible = |(_, key, offset, pixels): &&Live| {
            (0..2).any(|i| (offset[i] - at(key)[i]).abs() * pixels >= SUBPIXEL)
        };
        let due: BTreeSet<&Key> = live.iter().filter(visible).map(|l| &l.1).collect();
        live.iter().filter(|l| due.contains(&l.1)).map(|l| (l.0.clone(), at(&l.1))).collect()
    }
}

impl State {
    pub(super) fn move_reliefs(&mut self) {
        let live: Vec<Live> = (self.outputs.iter())
            .filter(|(name, o)| o.active && o.watched(self.exporter.is_streaming(name)) && self.moves(o))
            .filter_map(|(name, o)| {
                let (Content::Image(picture), Some((_, p))) = (&o.shown.content, &o.shown.relief) else {
                    return None;
                };
                let (key, pixels) =
                    ((picture.file.clone(), p.source, p.smoothing_ms), o.size.0.max(o.size.1));
                Some((name.clone(), key, o.offset, p.strength * pixels as f32))
            })
            .collect();
        for (name, at) in self.motion.step(&live, crate::inputs::pointer_over, Instant::now()) {
            if let Some(output) = self.outputs.get_mut(&name).filter(|o| o.offset != at) {
                (output.offset, output.dirty) = (at, true);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Moved = BTreeMap<String, [f32; 2]>;

    fn run(motion: &mut Motion, live: &[Live], over: &[&str], start: Instant) -> Moved {
        let pointer = |names: &[&str]| names.iter().any(|n| over.contains(n)).then_some([1.0, 0.0]);
        (1..=30).flat_map(|i| motion.step(live, pointer, start + Duration::from_millis(16 * i))).collect()
    }

    fn output(name: &str, file: &str, source: Source) -> Live {
        (name.to_owned(), (PathBuf::from(file), source, 100), [0.0; 2], 1e3)
    }

    #[test]
    fn each_image_follows_the_pointer_over_its_own_outputs() {
        let (mut motion, start) = (Motion::new(false), Instant::now());
        let live = [output("A", "a.png", Source::Pointer), output("B", "b.png", Source::Pointer)];
        let moved = run(&mut motion, &live, &["A"], start);
        assert!(moved.get("A").is_some_and(|at| at[0] > 0.9), "{moved:?}");
        assert!(!moved.contains_key("B"), "the pointer never entered B: {moved:?}");
        let spanned = [output("A", "a.png", Source::Pointer), output("C", "a.png", Source::Pointer)];
        let moved = run(&mut Motion::new(false), &spanned, &["C"], start);
        assert!(moved.contains_key("A") && moved.get("A") == moved.get("C"), "{moved:?}");
    }

    #[test]
    fn a_new_source_starts_where_its_image_is() {
        let (mut motion, start) = (Motion::new(false), Instant::now());
        let orbit = output("A", "a.png", Source::Auto);
        run(&mut motion, std::slice::from_ref(&orbit), &[], start);
        let (name, key, _, pixels) = output("B", "b.png", Source::Auto);
        let live = [orbit, (name, key, [-0.5, 0.5], pixels)];
        let joined = motion.step(&live, |_| None, start + Duration::from_millis(16 * 30));
        assert!(joined.iter().any(|(name, _)| name == "A"), "{joined:?}");
        assert!(joined.iter().all(|(name, _)| name != "B"), "B jumped: {joined:?}");
    }
}
