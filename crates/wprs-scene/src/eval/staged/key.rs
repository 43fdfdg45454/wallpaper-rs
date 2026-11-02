use super::Shade;
use crate::eval::{Content, Frame};
use crate::resolve::Context;
use std::hash::{DefaultHasher, Hasher};

fn floats(h: &mut DefaultHasher, v: &[f32]) {
    v.iter().for_each(|x| h.write_u32(x.to_bits()));
}

fn eye(h: &mut DefaultHasher, e: &crate::eye::Eye) {
    floats(h, &[e.fov, e.near, e.far, e.aspect]);
    [e.at, e.center, e.up].iter().for_each(|v| floats(h, v));
}

pub fn seal<'a>(cx: &Context<'a>, frame: &mut Frame<'a>) {
    if frame.placed.is_empty() {
        return;
    }
    for p in 0..frame.placed.len() {
        if frame.placed[p].stage == usize::MAX {
            super::lone(cx, frame, p);
        }
    }
    let wind = cx.scene.canvas.wind.as_ref().map_or([0.0; 2], |w| w.get(cx).0);
    let mut lights = DefaultHasher::new();
    for l in &frame.lamps {
        lights.write_u8(l.beam as u8);
        floats(&mut lights, &l.world.0);
        floats(&mut lights, &[l.tilt, l.intensity, l.radius, l.z, l.cone[0], l.cone[1], l.length]);
        floats(&mut lights, &l.color.0);
        let shadow = l.shadow.map_or([-1.0; 2], |s| [s.0.map_or(0.0, |r| r as f32), s.1]);
        floats(&mut lights, &shadow);
    }
    for c in [frame.ambient, frame.sky].iter().flatten() {
        floats(&mut lights, &c.0);
    }
    lights.write_u8(frame.quality as u8);
    let lights = lights.finish();
    for s in &mut frame.stages {
        let mut h = DefaultHasher::new();
        h.write_u64(lights);
        eye(&mut h, &s.eye);
        floats(&mut h, &[s.size[0], s.size[1], s.slices as f32, s.fade]);
        if let Some((e, k)) = &s.cross {
            eye(&mut h, e);
            floats(&mut h, &[*k]);
        }
        floats(&mut h, &s.planes);
        floats(&mut h, &s.world.0);
        let mut sways = false;
        for p in &frame.placed[s.models.clone()] {
            h.write_u64(p.model.digest);
            h.write_u64(p.pose.map_or(0, |q| q.generation));
            floats(&mut h, &p.matrix);
            h.write_u32(p.opacity.to_bits());
            h.write(p.env.unwrap_or_default().as_bytes());
            let looks = &frame.looks[p.looks.clone()];
            sways |= looks.iter().any(|l| l.shade == Shade::Foliage && l.extra[0] > 0.0);
            let lively = |k: u64| cx.scene.loaded.values().any(|d| d.key == k && d.animates());
            if looks.iter().any(|l| l.shade == Shade::Package && lively(l.package)) {
                h.write_u32(cx.t.to_bits());
                cx.note(crate::Wake::Frame);
            }
            let _ = std::fmt::Write::write_fmt(&mut Writer(&mut h), format_args!("{looks:?}"));
        }
        if sways && wind != [0.0; 2] && frame.quality > crate::Quality::Q0 {
            s.wind = wind;
            floats(&mut h, &[wind[0], wind[1], s.time]);
            cx.note(crate::Wake::Frame);
        }
        s.key = h.finish();
    }
    for d in &mut frame.draws {
        if let Content::Model { stage, key, .. } = &mut d.content {
            *key = frame.stages.get(*stage).map_or(0, |s| s.key);
        }
    }
}

struct Writer<'h>(&'h mut DefaultHasher);

impl std::fmt::Write for Writer<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0.write(s.as_bytes());
        Ok(())
    }
}
