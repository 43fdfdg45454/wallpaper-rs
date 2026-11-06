use crate::input::VideoCommand;
use crate::look::Playback;
use crate::runtime::Runtime;
use crate::stage::VIDEOS;
use crate::{Course, Kind, Layer, Scene, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Tape {
    pub asset: String,
    pub layer: Option<String>,
    pub at: f64,
    pub rate: f32,
    pub playing: bool,
    pub repeat: bool,
    pub start: f64,
    pub end: Option<f64>,
    pub jumps: u32,
    pub shown: bool,
    pub ended: bool,
    pub volume: f32,
    pub(crate) held: Option<bool>,
    pub(crate) speed: Option<f32>,
}

fn named(v: &Value<String>, asset: &str) -> bool {
    match v {
        Value::Fixed(id) => id == asset,
        Value::Bound(b) => b.map.iter().flat_map(|m| m.values()).any(|id| id == asset),
    }
}

fn shows<'a>(scene: &Scene, layer: &'a Layer, asset: &str) -> Option<Option<&'a Playback>> {
    match &layer.kind {
        _ if !owned(scene, &layer.kind).is_empty() => None,
        Kind::Video { video: v, playback, .. } | Kind::Image { image: v, playback, .. }
            if named(v, asset) =>
        {
            Some(playback.as_ref())
        }
        _ => None,
    }
}

pub(crate) fn owned<'a>(scene: &Scene, kind: &'a Kind) -> Vec<&'a String> {
    let Kind::Video { video, playback: Some(p), .. } = kind else { return Vec::new() };
    let mut names: Vec<&String> = match video {
        Value::Fixed(id) => vec![id],
        Value::Bound(b) => b.map.iter().flat_map(|m| m.values()).collect(),
    };
    names.sort();
    names.dedup();
    names.retain(|id| p.own_clock && !scene.pools.contains_key(*id));
    names
}

pub fn lane(scene: &Scene, layer: &Layer, asset: &str) -> Option<String> {
    let own = owned(scene, &layer.kind).into_iter().any(|id| id == asset);
    own.then(|| format!("{asset}@{}", layer.id))
}

impl Scene {
    pub fn lanes(&self) -> Vec<(String, String)> {
        let own = self.layers().flat_map(|l| owned(self, &l.kind).into_iter().map(move |a| (l, a)));
        own.map(|(l, asset)| (format!("{asset}@{}", l.id), asset.clone())).collect()
    }
}

fn fresh(id: &str, owner: Option<(&Layer, Option<&Playback>)>) -> Tape {
    let p = owner.and_then(|(_, p)| p).cloned().unwrap_or_default();
    let (start, end) = (f64::from(p.start), p.end.map(f64::from));
    let (layer, held, speed) = (owner.map(|(l, _)| l.id.clone()), None, None);
    let (jumps, shown, ended, playing, rate) = (0, true, false, true, 1.0);
    let asset = id.to_owned();
    Tape {
        asset,
        layer,
        at: start,
        rate,
        playing,
        repeat: p.repeat,
        start,
        end,
        jumps,
        shown,
        ended,
        volume: 1.0,
        held,
        speed,
    }
}

pub(crate) fn played(l: &Layer) -> Option<&Playback> {
    match &l.kind {
        Kind::Video { playback, .. } | Kind::Image { playback, .. } => playback.as_ref(),
        _ => None,
    }
}

pub(crate) fn tapes(scene: &Scene) -> Vec<Tape> {
    let videos = scene.assets.iter().filter(|(_, a)| VIDEOS.contains(&a.extension().as_str()));
    let tape = fresh;
    let shared = videos.map(|(id, _)| tape(id, scene.layers().find_map(|l| Some((l, shows(scene, l, id)?)))));
    let own = scene.layers().flat_map(|l| {
        let playback = played(l);
        let keys = owned(scene, &l.kind).into_iter().map(|a| format!("{a}@{}", l.id));
        keys.map(move |key| tape(&key, Some((l, playback)))).collect::<Vec<_>>()
    });
    shared.collect::<Vec<_>>().into_iter().chain(own).collect()
}

impl Runtime {
    pub fn tapes(&self) -> &[Tape] {
        &self.tapes
    }

    pub(crate) fn film_files(&mut self, sets: &BTreeMap<String, crate::LayerOverride>) {
        let found = |id: &String| crate::copies::found(&self.scene, &self.spawned, id);
        let video = |l: &&Layer| matches!(l.kind, Kind::Video { .. });
        let shown = sets.iter().filter_map(|(id, s)| Some((found(id).filter(video)?, s.file.as_ref()?)));
        let mut wanted: Vec<(&Layer, &String)> = Vec::new();
        shown.for_each(|(l, f)| wanted.extend((!wanted.iter().any(|(_, g)| *g == f)).then_some((l, f))));
        let user = |t: &Tape| t.asset.starts_with(crate::USER_FILES);
        let kept = |t: &Tape| {
            !user(t) || wanted.iter().any(|(l, f)| **f == t.asset && t.layer.as_ref() == Some(&l.id))
        };
        let fresh: Vec<Tape> = wanted
            .iter()
            .filter(|(l, f)| !self.tapes.iter().any(|t| t.asset == **f && t.layer.as_ref() == Some(&l.id)))
            .map(|(l, f)| fresh(f, Some((l, played(l)))))
            .collect();
        self.tapes.retain(kept);
        self.tapes.extend(fresh);
    }

    pub fn fill_videos(&self, out: &mut Vec<Course>) {
        out.clear();
        for (i, t) in self.tapes.iter().enumerate() {
            let Some(id) = &t.layer else { continue };
            let same = |o: &&Tape| o.layer == t.layer;
            let lead = self.tapes.iter().filter(same).find(|o| o.shown).or(self.tapes.iter().find(same));
            if lead.is_some_and(|o| !std::ptr::eq(o, &self.tapes[i])) {
                continue;
            }
            let duration = self.length(&t.asset) as f32;
            let (time, playing, speed) = (t.at as f32, t.playing && !t.ended, t.rate);
            out.push(Course { id: id.clone(), time, duration, playing, speed });
        }
        self.puppet_courses(out);
    }

    pub fn take_ended(&mut self) -> Vec<String> {
        std::mem::take(&mut self.ended)
    }

    pub(crate) fn length(&self, asset: &str) -> f64 {
        self.lengths.get(asset).map_or(f64::INFINITY, |s| f64::from(*s))
    }

    pub(crate) fn steer(&mut self, videos: &BTreeMap<String, crate::input::VideoControl>) {
        for (layer, control) in videos {
            let lengths = &self.lengths;
            for t in self.tapes.iter_mut().filter(|t| t.layer.as_ref() == Some(layer)) {
                Self::steered(t, lengths, control);
            }
        }
    }

    fn steered(t: &mut Tape, lengths: &BTreeMap<String, f32>, control: &crate::input::VideoControl) {
        let last = lengths.get(&t.asset).map_or(f64::INFINITY, |s| f64::from(*s));
        let jump = control.commands.iter().rev().find_map(|c| match c {
            VideoCommand::Seek(s) => Some(f64::from(*s)),
            _ => None,
        });
        for command in &control.commands {
            match *command {
                VideoCommand::Play if t.ended => {
                    (t.at, t.ended, t.held, t.jumps) = (t.start, false, Some(true), t.jumps + 1)
                }
                VideoCommand::Play => t.held = Some(true),
                VideoCommand::Pause => t.held = Some(false),
                VideoCommand::Stop => {
                    (t.at, t.ended, t.held, t.jumps) = (t.start, false, Some(false), t.jumps + 1)
                }
                VideoCommand::Rate(r) => t.speed = Some(crate::roll::playable(r)),
                VideoCommand::Volume(v) => {
                    t.volume = if v.is_finite() { v.clamp(0.0, 1.0) } else { t.volume }
                }
                VideoCommand::Seek(_) | VideoCommand::Weight(_) => {}
            }
        }
        if let Some(at) = jump {
            (t.at, t.ended, t.jumps) = (at.clamp(0.0, last.min(1e5)), false, t.jumps + 1);
        }
    }
}
