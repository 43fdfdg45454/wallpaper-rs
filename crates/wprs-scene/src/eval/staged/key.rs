use crate::eval::{Content, Frame};
use crate::resolve::Context;
use std::hash::{DefaultHasher, Hasher};

fn floats(h: &mut DefaultHasher, v: &[f32]) {
    v.iter().for_each(|x| h.write_u32(x.to_bits()));
}

pub fn seal<'a>(cx: &Context<'a>, frame: &mut Frame<'a>) {
    if frame.placed.is_empty() {
        return;
    }
    let eye = super::eye(cx);
    for p in 0..frame.placed.len() {
        if frame.placed[p].stage == usize::MAX {
            super::lone(frame, p, eye);
        }
    }
    let mut lights = DefaultHasher::new();
    for l in &frame.lamps {
        lights.write_u8(l.beam as u8);
        floats(&mut lights, &l.world.0);
        floats(&mut lights, &[l.tilt, l.intensity, l.radius, l.z, l.cone[0], l.cone[1], l.length]);
        floats(&mut lights, &l.color.0);
    }
    for c in [frame.ambient, frame.sky].iter().flatten() {
        floats(&mut lights, &c.0);
    }
    lights.write_u8(frame.quality as u8);
    let lights = lights.finish();
    for s in &mut frame.stages {
        let mut h = DefaultHasher::new();
        h.write_u64(lights);
        let e = &s.eye;
        floats(&mut h, &[e.fov, e.near, e.far, e.aspect]);
        [e.at, e.center, e.up, [s.size[0], s.size[1], s.slices as f32]]
            .iter()
            .for_each(|v| floats(&mut h, v));
        floats(&mut h, &s.planes);
        floats(&mut h, &s.world.0);
        for p in &frame.placed[s.models.clone()] {
            h.write_u64(p.model.digest);
            floats(&mut h, &p.matrix);
            h.write_u32(p.opacity.to_bits());
            h.write(p.env.unwrap_or_default().as_bytes());
            let _ = std::fmt::Write::write_fmt(
                &mut Writer(&mut h),
                format_args!("{:?}", &frame.looks[p.looks.clone()]),
            );
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
