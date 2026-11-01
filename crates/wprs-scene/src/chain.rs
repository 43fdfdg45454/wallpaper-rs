use crate::effects::{Descriptor, Fuse, PRELUDE, SLOTS, descriptor};
use crate::{Class, EffectUse, Value};
use std::fmt::Write;
use std::ops::Range;

pub const CHAIN_ENTRY: &str = "fs_chain";

pub fn effect_source(chain: &[&Descriptor]) -> String {
    let mut s = String::from(PRELUDE);
    for d in chain {
        s.push('\n');
        s.push_str(&d.wgsl);
    }
    let entry = match chain {
        [] => return s,
        [d] if d.fuse.is_none() => return s,
        [d] => format!("fs_{}", d.id),
        _ => CHAIN_ENTRY.into(),
    };
    let n = chain.len();
    let _ = writeln!(s, "\n@fragment\nfn {entry}(in: VertexOut) -> @location(0) vec4<f32> {{");
    let _ = writeln!(s, "    let q{n} = in.uv;");
    let mut bases = Vec::with_capacity(n);
    chain.iter().fold(0, |base, d| {
        bases.push(base);
        base + d.params.len()
    });
    for (k, d) in chain.iter().enumerate().rev() {
        let (id, base, next) = (&d.id, bases[k], k + 1);
        let _ = match d.fuse {
            Some(Fuse::Uv) => writeln!(
                s,
                "    fx_base = {base}u;\n    let q{k} = select(uv_{id}(q{next}), OUTSIDE_UV, q{next}.x <= OUTSIDE_UV.x);"
            ),
            _ => writeln!(s, "    let q{k} = q{next};"),
        };
    }
    s.push_str("    var c = select(tap(q0), vec4<f32>(0.0), q0.x <= OUTSIDE_UV.x);\n");
    for (k, d) in chain.iter().enumerate().filter(|(_, d)| d.fuse == Some(Fuse::Color)) {
        let _ = writeln!(s, "    fx_base = {}u;\n    c = color_{}(c, q{});", bases[k], d.id, k + 1);
    }
    s.push_str("    return c;\n}\n");
    s
}

fn fusable(e: &EffectUse) -> Option<&'static Descriptor> {
    let pictured = |d: &Descriptor| d.params.iter().any(|p| p.kind.asset() && e.params.contains_key(&p.id));
    let fixed = e.mask.is_none() && e.enabled == Value::Fixed(true);
    descriptor(&e.effect).filter(|d| fixed && !pictured(d) && d.fuse.is_some() && d.class() == Class::Light)
}

pub fn runs(effects: &[EffectUse]) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut k = 0;
    std::iter::from_fn(move || {
        while k < effects.len() {
            let (mut n, mut slots) = (0, 0);
            while let Some(d) = effects.get(k + n).and_then(fusable) {
                slots += d.params.len();
                if slots > SLOTS {
                    break;
                }
                n += 1;
            }
            let start = k;
            k += n.max(1);
            if n > 1 {
                return Some(start..start + n);
            }
        }
        None
    })
}

pub fn fusion(effects: &[EffectUse]) -> Vec<Range<usize>> {
    runs(effects).collect()
}
