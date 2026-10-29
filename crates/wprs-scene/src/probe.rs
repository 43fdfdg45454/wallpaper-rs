use crate::signal::{CLOCKS, Smooth};
use crate::validate::{Check, Issue};
use crate::value::{Band, Binding, Param, Value, Vec2, Vec3};
use crate::{Color, Ease};

pub(crate) trait Probe: Sized {
    const TEXT: bool = false;
    const NUMERIC: bool = false;
    const SCALAR: bool = false;
    const PARAM: bool = false;
    const COLOR: bool = false;
    const FLAG: bool = false;
    const PARTS: &'static [&'static str] = &[];
    fn probe(_: &mut Check, _: &str, _: &Self) {}
}

impl Probe for f32 {
    const NUMERIC: bool = true;
    const SCALAR: bool = true;
    fn probe(c: &mut Check, path: &str, v: &f32) {
        c.range(path, *v, -1e6, 1e6);
    }
}

impl Probe for bool {
    const FLAG: bool = true;
}

impl Probe for String {
    const TEXT: bool = true;
}

impl Probe for Vec2 {
    const NUMERIC: bool = true;
    const PARTS: &'static [&'static str] = &["x", "y"];
    fn probe(c: &mut Check, path: &str, v: &Vec2) {
        v.0.iter().enumerate().for_each(|(i, x)| c.range(&format!("{path}/{i}"), *x, -1e6, 1e6));
    }
}

impl Probe for Vec3 {
    const NUMERIC: bool = true;
    fn probe(c: &mut Check, path: &str, v: &Vec3) {
        v.0.iter().enumerate().for_each(|(i, x)| c.range(&format!("{path}/{i}"), *x, -1e6, 1e6));
    }
}

impl Probe for Color {
    const COLOR: bool = true;
    const NUMERIC: bool = true;
    const PARTS: &'static [&'static str] = &["r", "g", "b", "a"];
    fn probe(c: &mut Check, path: &str, v: &Color) {
        c.color(path, *v);
    }
}

impl Probe for Param {
    const NUMERIC: bool = true;
    const PARAM: bool = true;
    fn probe(c: &mut Check, path: &str, v: &Param) {
        match v {
            Param::Number(n) => f32::probe(c, path, n),
            Param::Vec2(v) => Vec2::probe(c, path, &Vec2(*v)),
            Param::Color(color) => c.color(path, *color),
            Param::Bool(_) | Param::Text(_) => {}
        }
    }
}

impl Check<'_> {
    pub(crate) fn value<T: Probe>(&mut self, path: &str, value: &Value<T>) {
        let b = match value {
            Value::Fixed(v) => return T::probe(self, path, v),
            Value::Bound(b) => b,
        };
        if b.sources() != 1 {
            self.issues.push(Issue::Source(path.into()));
        }
        if let Some(id) = &b.prop {
            self.property(path, id, false);
        }
        let clock = b.clock.as_deref().is_some_and(|c| T::TEXT || CLOCKS.contains(&c));
        let dated = b.date.is_some() && !T::TEXT;
        let misplaced = (b.map.is_some() && b.prop.is_none()) || (b.clock.is_some() && !clock) || dated;
        let heard = b.audio.is_some() || b.cursor.is_some();
        if misplaced || (T::TEXT && heard) || (b.layer.is_some() && !T::PARAM) {
            self.issues.push(Issue::Type(path.into()));
        }
        self.uses(b.clock.is_some() && !T::TEXT, "clock_values");
        self.uses(b.date.is_some(), "clock_dates");
        self.keys(path, b);
        self.called(path, b);
        if let Some(id) = &b.timeline {
            let known = self.scene.timelines.iter().any(|t| &t.id == id);
            self.known(&format!("{path}/timeline"), "timeline", id, known);
        }
        self.signal(path, b);
        for (name, v) in [("scale", b.scale), ("offset", b.offset)] {
            v.into_iter().for_each(|v| self.range(&format!("{path}/{name}"), v, -1e6, 1e6));
        }
        if let Some([lo, hi]) = b.clamp {
            self.range(&format!("{path}/clamp/0"), lo, -1e6, 1e6);
            self.range(&format!("{path}/clamp/1"), hi, lo, 1e6);
        }
        for (name, track) in b.tracks() {
            let Some(track) = track else { continue };
            self.uses(true, "tracks");
            match T::PARTS.contains(&name) {
                true => self.value(&format!("{path}/{name}"), track),
                false => self.issues.push(Issue::Type(format!("{path}/{name}"))),
            }
        }
        if let Some(base) = &b.base {
            self.uses(true, "relative_values");
            match T::NUMERIC {
                true => T::probe(self, &format!("{path}/base"), base),
                false => self.issues.push(Issue::Type(format!("{path}/base"))),
            }
        }
    }

    fn keys<T: Probe>(&mut self, path: &str, b: &Binding<T>) {
        let Some(keys) = &b.keys else { return };
        self.keys_sorted(&format!("{path}/keys"), keys.iter().map(|k| k.t));
        for (i, k) in keys.iter().enumerate() {
            T::probe(self, &format!("{path}/keys/{i}/1"), &k.v);
            if let Ease::Bezier(curve) = k.ease {
                for (j, v) in curve.into_iter().enumerate() {
                    let [lo, hi] = if j % 2 == 0 { [0.0, 1.0] } else { [-1e3, 1e3] };
                    self.range(&format!("{path}/keys/{i}/2/{j}"), v, lo, hi);
                }
            }
            let Some(h) = k.handles else { continue };
            self.uses(true, "tangents");
            if !T::SCALAR {
                self.issues.push(Issue::Type(format!("{path}/keys/{i}/2")));
            }
            let before = i.checked_sub(1).map_or(0.0, |j| k.t - keys[j].t);
            let after = keys.get(i + 1).map_or(0.0, |n| n.t - k.t);
            for (name, handle, room) in [("in", h.before, before), ("out", h.out, after)] {
                let Some([dt, dv]) = handle else { continue };
                self.range(&format!("{path}/keys/{i}/2/{name}/0"), dt, 0.0, room.max(0.0));
                self.range(&format!("{path}/keys/{i}/2/{name}/1"), dv, -1e6, 1e6);
            }
        }
    }

    fn signal<T>(&mut self, path: &str, b: &Binding<T>) {
        let of = b.of.unwrap_or(64);
        if let Some(of) = b.of {
            self.uses(true, "audio_ranges");
            if ![16, 32, 64].contains(&of) {
                self.issues.push(Issue::Range {
                    path: format!("{path}/of"),
                    value: of as f32,
                    min: 16.0,
                    max: 64.0,
                });
            }
        }
        let top = of.saturating_sub(1) as f32;
        match b.audio {
            Some(Band::Index(i)) => self.range(&format!("{path}/audio"), i as f32, 0.0, top),
            Some(Band::Range([lo, hi])) => {
                self.uses(true, "audio_ranges");
                self.range(&format!("{path}/audio/0"), lo as f32, 0.0, top);
                self.range(&format!("{path}/audio/1"), hi as f32, lo as f32, top);
            }
            _ => {}
        }
        let ranged = matches!(b.audio, Some(Band::Range(_)));
        if (b.of.is_some() && b.audio.is_none()) || (b.reduce.is_some() && !ranged) {
            self.issues.push(Issue::Type(path.into()));
        }
        self.uses(b.reduce.is_some(), "audio_ranges");
        if let Some([lo, hi]) = b.bounds {
            self.uses(true, "shaping");
            self.range(&format!("{path}/bounds/0"), lo, -1e6, 1e6);
            self.range(&format!("{path}/bounds/1"), hi, lo.next_up(), 1e6);
        }
        if let Some(e) = b.exponent {
            self.uses(true, "shaping");
            self.range(&format!("{path}/exponent"), e, 0.1, 10.0);
        }
        if let Some(smooth) = b.smooth {
            self.uses(matches!(smooth, Smooth::Apart { .. }), "shaping");
            let [up, down] = smooth.times();
            self.range(&format!("{path}/smooth"), up, 0.0, 60.0);
            self.range(&format!("{path}/smooth"), down, 0.0, 60.0);
        }
    }
}
