use super::{ClipMap, Head, Puppet};
use crate::resolve::Context;
use crate::rigged::Rigged;
use crate::{AnimClip, Course, Fired, Loop, MAX_ANIMATION_LAYERS, MAX_FIRED, MeshLayer, VideoCommand, Wake};

pub(super) fn map(rig: &Rigged, c: &AnimClip) -> ClipMap {
    let motion = match &c.animation {
        Some(n) => rig.motions.iter().position(|m| &m.name == n),
        None if !c.tracked() && !rig.motions.is_empty() => Some(0),
        None => None,
    };
    let bones = c.bones.keys().map(|b| rig.joint(b).map(|i| i as u16)).collect();
    let shapes = c.shapes.keys().map(|s| rig.shape(s).map(|i| i as u16)).collect();
    let order = c.order.keys().map(|p| rig.part(p).map(|i| i as u16)).collect();
    let end = c.to.unwrap_or_else(|| motion.map_or(0.0, |m| rig.motions[m].length).max(c.end()));
    ClipMap { motion, bones, shapes, order, length: (end - c.from).max(0.0) }
}

pub(super) fn head(rig: &Rigged, m: &MeshLayer, layer: Option<usize>, clip: Option<usize>) -> Option<Head> {
    let animation = layer.map(|i| &m.rig.animations[i]);
    let clip = match animation {
        Some(a) => m.rig.clips.iter().position(|c| c.id == a.clip)?,
        None => clip?,
    };
    let bones = animation.map_or(&[][..], |a| &a.bones[..]);
    let mask = match bones.is_empty() {
        true => Vec::new(),
        false => rig.joints.iter().map(|j| bones.contains(&j.name)).collect(),
    };
    let (t, fade, ended, off, held, rate, weight, speed) = (0.0, 0.0, false, false, None, None, None, 1.0);
    Some(Head { clip, layer, t, fade, ended, off, held, rate, weight, mask, speed, fresh: true })
}

fn toward(fade: f32, on: bool, dt: f32, (up, down): (f32, f32)) -> f32 {
    match on {
        true if up > 0.0 => (fade + dt / up).min(1.0),
        true => 1.0,
        false if down > 0.0 => (fade - dt / down).max(0.0),
        false => 0.0,
    }
}

impl Puppet {
    pub(crate) fn step(
        &mut self,
        cx: &Context,
        m: &MeshLayer,
        dt: f32,
        (ended, heard): (&mut Vec<String>, &mut Vec<(Fired, f32)>),
    ) {
        let mut wake = Wake::Idle;
        for i in 0..self.heads.len() {
            let first = !self.heads[..i].iter().any(|o| o.clip == self.heads[i].clip);
            let h = &mut self.heads[i];
            let a = h.layer.map(|i| &m.rig.animations[i]);
            let (clip, map) = (&m.rig.clips[h.clip], &self.clips[h.clip]);
            let on = (h.held == Some(true) || a.is_none_or(|a| a.enabled.get(cx))) && !h.off;
            let blends = a.map_or((0.0, 0.0), |a| (a.blend_in, a.blend_out));
            let was = h.fade;
            h.fade = toward(h.fade, on, dt, blends);
            h.speed = h.rate.unwrap_or_else(|| a.map_or(1.0, |a| a.rate.get(cx))).clamp(0.0, 16.0);
            let target = if on { 1.0 } else { 0.0 };
            let rolls = h.held.unwrap_or(true) && !h.ended && h.speed > 0.0 && map.length > 0.0;
            if h.fade != target || (rolls && h.fade > 0.0) {
                wake = Wake::Frame;
            }
            if !rolls || (h.fade <= 0.0 && was <= 0.0) {
                continue;
            }
            let (from, length) = (h.t, f64::from(map.length));
            h.t += f64::from(dt * h.speed);
            let over = clip.repeat == Loop::Once && h.t >= length;
            if over {
                (h.t, h.ended, h.off) = (length, true, blends.1 > 0.0);
            }
            if dt > 0.0 && first && !clip.events.is_empty() {
                super::cues::crossed((&self.layer, clip), length, (from, h.t, h.fresh), h.speed, heard);
                h.fresh = false;
            }
            if over && ended.len() < MAX_FIRED {
                ended.push(format!("{}#{}", self.layer, clip.id));
            }
            if clip.repeat != Loop::Once {
                h.t = h.t.rem_euclid(super::cues::period(clip.repeat, length));
            }
        }
        self.wake = wake;
    }

    pub(crate) fn steer(&mut self, m: &MeshLayer, clip: &str, commands: &[VideoCommand]) {
        let Some(c) = m.rig.clips.iter().position(|x| x.id == clip) else { return };
        if !self.heads.iter().any(|h| h.clip == c) && self.heads.len() < 2 * MAX_ANIMATION_LAYERS {
            self.heads.extend(head(&self.rig, m, None, Some(c)));
        }
        let length = f64::from(self.clips[c].length);
        for h in self.heads.iter_mut().filter(|h| h.clip == c) {
            for command in commands {
                match *command {
                    VideoCommand::Play if h.ended => {
                        (h.t, h.ended, h.off, h.held, h.fresh) = (0.0, false, false, Some(true), true)
                    }
                    VideoCommand::Play => (h.off, h.held) = (false, Some(true)),
                    VideoCommand::Pause => h.held = Some(false),
                    VideoCommand::Stop => {
                        (h.t, h.ended, h.off, h.held, h.fresh) = (0.0, false, true, Some(false), true)
                    }
                    VideoCommand::Seek(s) => {
                        (h.t, h.ended, h.fresh) = (f64::from(s).clamp(0.0, length), false, s <= 0.0)
                    }
                    VideoCommand::Rate(r) => h.rate = Some(r.clamp(0.0, 16.0)),
                    VideoCommand::Weight(w) => h.weight = Some(w.clamp(0.0, 1.0)),
                }
            }
        }
        self.dirty = true;
    }

    pub(crate) fn courses(&self, m: &MeshLayer, out: &mut Vec<Course>) {
        for (i, h) in self.heads.iter().enumerate() {
            if self.heads[..i].iter().any(|o| o.clip == h.clip) {
                continue;
            }
            let (clip, length) = (&m.rig.clips[h.clip], self.clips[h.clip].length);
            out.push(Course {
                id: format!("{}#{}", self.layer, clip.id),
                time: crate::local(h.t as f32, length, clip.repeat),
                duration: length,
                playing: h.held.unwrap_or(true) && !h.ended && h.fade > 0.0,
                speed: h.speed,
            });
        }
    }
}
