use crate::resolve::Context;
use crate::{Order, Runtime, Scene, Sound, Value, Voice, Wake};
use std::collections::{BTreeMap, BTreeSet};

const WALK: u32 = 4096;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Playlist {
    pub lengths: Vec<f64>,
    pub order: Order,
    pub pause: [f64; 2],
    pub repeat: bool,
    pub seed: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Spot {
    Clip { index: usize, offset: f64, left: f64 },
    Gap(f64),
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Walk {
    k: u64,
    start: f64,
    last: Option<usize>,
}

fn mix(mut x: u64) -> u64 {
    x = (x ^ x >> 30).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ x >> 27).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ x >> 31
}

impl Playlist {
    pub fn of(sound: &Sound, lengths: &BTreeMap<String, f32>, seed: u64) -> Playlist {
        let lengths = sound.clips().map(|id| f64::from(lengths.get(id).copied().unwrap_or(0.0))).collect();
        let pause = sound.pause.map_or([0.0; 2], |p| p.map(f64::from));
        Playlist { lengths, order: sound.mode.unwrap_or_default(), pause, repeat: sound.repeat, seed }
    }

    fn item(&self, k: u64, last: Option<usize>) -> (usize, f64) {
        let (n, h) = (self.lengths.len(), mix(self.seed ^ mix(k)));
        let clip = match self.order {
            Order::Sequence => (k % n as u64) as usize,
            Order::Random if n > 1 => {
                let c = (h % (n as u64 - 1)) as usize;
                if last.is_some_and(|l| c >= l) { c + 1 } else { c }
            }
            Order::Random => 0,
        };
        let u = (mix(h) >> 11) as f64 / (1u64 << 53) as f64;
        (clip, self.pause[0] + (self.pause[1] - self.pause[0]) * u)
    }

    pub fn spot(&self, at: f64, walk: &mut Walk) -> Spot {
        let n = self.lengths.len() as u64;
        if n == 0 || at < 0.0 || self.lengths.iter().any(|l| *l <= 0.0) {
            return Spot::End;
        }
        if at < walk.start {
            *walk = Walk::default();
        }
        let items = match (self.repeat, self.order) {
            (true, _) => u64::MAX,
            (false, Order::Random) => 1,
            (false, Order::Sequence) => n,
        };
        for _ in 0..WALK {
            if walk.k >= items {
                return Spot::End;
            }
            let (index, gap) = self.item(walk.k, walk.last);
            let end = walk.start + self.lengths[index];
            if at < end {
                return Spot::Clip { index, offset: at - walk.start, left: end - at };
            }
            if at < end + gap && walk.k + 1 < items {
                return Spot::Gap(end + gap - at);
            }
            *walk = Walk { k: walk.k + 1, start: end + gap, last: Some(index) };
        }
        Spot::Gap(0.0)
    }
}

pub(crate) fn listened(scene: &Scene, scripts: bool, sound: bool) -> BTreeSet<String> {
    let all = scene.timelines.iter().flat_map(|t| &t.events).map(|e| e.name.clone());
    let named = scene.timelines.iter().filter_map(|t| t.trigger.as_deref());
    let named = named.chain(scene.sounds().filter_map(|s| s.play_on.as_deref()).filter(|_| sound));
    let named = named.filter_map(|t| t.strip_prefix("event:")).map(str::to_owned);
    if scripts && !scene.scripts.is_empty() { all.collect() } else { named.collect() }
}

pub(crate) fn onsets(scene: &Scene) -> Vec<Option<f64>> {
    scene.sounds().map(|s| (s.play_on.as_deref() == Some("start")).then_some(0.0)).collect()
}

fn stopped(s: &Sound, cx: &Context) -> bool {
    let id = s.play.binding().and_then(|b| b.timeline.as_ref());
    id.is_some_and(|id| cx.timelines.iter().any(|(tl, head)| &tl.id == id && !head.playing))
}

fn edge(s: &Sound, cx: &Context, t: f64) -> Option<f64> {
    let b = s.play.binding()?;
    let keys = b.keys.as_ref()?;
    let p = cx.keys_time(b, keys.last().map_or(0.0, |k| k.t));
    let i = keys.partition_point(|k| k.t <= p);
    let start = (0..i).rev().take_while(|&j| keys[j].v).last()?;
    Some(t - f64::from((p - keys[start].t) / cx.rate(b)))
}

impl Runtime {
    pub fn set_listeners(&mut self, scripts: bool, sound: bool) {
        self.listened = listened(&self.scene, scripts, sound);
    }

    pub fn set_lengths(&mut self, lengths: BTreeMap<String, f32>) {
        self.lengths.clone_from(&lengths);
        let seed = |i: usize| self.seed ^ mix(i as u64 + 1);
        self.lists =
            self.scene.sounds().enumerate().map(|(i, s)| Playlist::of(s, &lengths, seed(i))).collect();
    }

    pub fn playlists(&self) -> &[Playlist] {
        &self.lists
    }

    pub fn voices(&self) -> (Vec<Voice>, Wake) {
        let (cx, lists, t) = (self.context(), &self.lists, self.time());
        let mut gates = self.gates.borrow_mut();
        let mut voices = Vec::new();
        for (i, s) in self.scene.sounds().enumerate() {
            let mut voice = s.voice(&cx);
            let gate = &mut gates[i];
            *gate = match voice.play {
                false => None,
                true if s.play == Value::Fixed(true) => Some(0.0),
                true if stopped(s, &cx) && gate.is_some() => *gate,
                true => edge(s, &cx, t).or(*gate).or(Some(t)),
            };
            let onset = if s.play_on.is_some() { self.onsets[i].filter(|_| gate.is_some()) } else { *gate };
            let onset = self.played.taken(i).map_or(onset, |k| k.since.filter(|_| voice.play));
            voice.volume *= self.played.volume(i);
            voice.at = onset.map_or(0.0, |o| (t - o).max(0.0));
            let list = &lists[i];
            if voice.play && !list.repeat && list.lengths.iter().all(|l| *l > 0.0) {
                match finish(list, voice.at) {
                    Some(left) => cx.note(Wake::At(left)),
                    None => voice.play = false,
                }
            }
            voice.play &= onset.is_some();
            voices.push(voice);
        }
        *self.played.heard.borrow_mut() = voices.iter().map(|v| v.play).collect();
        (voices, cx.wake.get().min(self.cue_wake()))
    }
}

fn finish(list: &Playlist, at: f64) -> Option<f64> {
    let (mut walk, mut x) = (Walk::default(), at);
    loop {
        match list.spot(x, &mut walk) {
            Spot::Clip { left, .. } | Spot::Gap(left) if left > 0.0 => x += left,
            _ => return (x > at).then_some(x - at),
        }
    }
}
