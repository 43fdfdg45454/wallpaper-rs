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
    pub(crate) held: Option<bool>,
    pub(crate) speed: Option<f32>,
}

fn named(v: &Value<String>, asset: &str) -> bool {
    match v {
        Value::Fixed(id) => id == asset,
        Value::Bound(b) => b.map.iter().flat_map(|m| m.values()).any(|id| id == asset),
    }
}

fn shows<'a>(layer: &'a Layer, asset: &str) -> Option<Option<&'a Playback>> {
    match &layer.kind {
        Kind::Video { video: v, playback, .. } | Kind::Image { image: v, playback, .. }
            if named(v, asset) =>
        {
            Some(playback.as_ref())
        }
        _ => None,
    }
}

pub(crate) fn tapes(scene: &Scene) -> Vec<Tape> {
    let videos = scene.assets.iter().filter(|(_, a)| VIDEOS.contains(&a.extension().as_str()));
    let tape = |id: &String| {
        let owner = scene.layers().find_map(|l| Some((l, shows(l, id)?)));
        let p = owner.and_then(|(_, p)| p).cloned().unwrap_or_default();
        let (start, end) = (f64::from(p.start), p.end.map(f64::from));
        let (layer, held, speed) = (owner.map(|(l, _)| l.id.clone()), None, None);
        let (jumps, shown, ended, playing, rate) = (0, false, false, true, 1.0);
        Tape {
            asset: id.clone(),
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
            held,
            speed,
        }
    };
    videos.map(|(id, _)| tape(id)).collect()
}

impl Runtime {
    pub fn tapes(&self) -> &[Tape] {
        &self.tapes
    }

    pub fn fill_videos(&self, out: &mut Vec<Course>) {
        out.clear();
        for t in self.tapes.iter() {
            let Some(id) = &t.layer else { continue };
            let duration = self.length(&t.asset) as f32;
            let (time, playing, speed) = (t.at as f32, t.playing && !t.ended, t.rate);
            out.push(Course { id: id.clone(), time, duration, playing, speed });
        }
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
            let Some(t) = self.tapes.iter_mut().find(|t| t.layer.as_ref() == Some(layer)) else { continue };
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
                    VideoCommand::Rate(r) => t.speed = Some(r.clamp(0.0, 16.0)),
                    VideoCommand::Seek(_) => {}
                }
            }
            if let Some(at) = jump {
                (t.at, t.ended, t.jumps) = (at.clamp(0.0, last.min(1e5)), false, t.jumps + 1);
            }
        }
    }
}
