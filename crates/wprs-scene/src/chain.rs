use crate::effects::{Descriptor, FX_SLOTS, Fuse, MASK_SLOTS, MaskMode, PRELUDE, descriptor};
use crate::{Class, EffectUse, Value};
use std::fmt::Write;
use std::ops::Range;

pub const CHAIN_ENTRY: &str = "fs_chain";

pub fn effect_source(chain: &[&Descriptor]) -> String {
    let mut s = String::from(PRELUDE);
    for (k, d) in chain.iter().enumerate() {
        if chain[..k].iter().all(|o| o.id != d.id) {
            s.push('\n');
            s.push_str(&d.wgsl);
        }
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
    let masks = if n > 1 { MASK_SLOTS } else { 0 };
    let mut bases = Vec::with_capacity(n);
    chain.iter().fold(0, |base, d| {
        bases.push(base);
        base + d.params.len() + masks
    });
    let at = |k: usize| {
        let mask = if masks > 0 { bases[k] + chain[k].params.len() } else { 0 };
        format!("    fx_base = {}u;\n    fx_mask = {mask}u;", bases[k])
    };
    for (k, d) in chain.iter().enumerate().rev() {
        let (id, next) = (&d.id, k + 1);
        let _ = match d.fuse {
            Some(Fuse::Uv) => writeln!(
                s,
                "{}\n    let q{k} = select(uv_{id}(q{next}), OUTSIDE_UV, q{next}.x <= OUTSIDE_UV.x);",
                at(k)
            ),
            _ => writeln!(s, "    let q{k} = q{next};"),
        };
    }
    s.push_str("    var c = select(tap(q0), vec4<f32>(0.0), q0.x <= OUTSIDE_UV.x);\n");
    for (k, d) in chain.iter().enumerate().filter(|(_, d)| d.fuse == Some(Fuse::Color)) {
        let (id, q) = (&d.id, k + 1);
        let _ = match masks > 0 && d.mask_mode == MaskMode::Mix {
            true => writeln!(
                s,
                "{}\n    let c{k} = c;\n    c = mix(c{k}, color_{id}(c, q{q}), mask_at(q{q}));",
                at(k)
            ),
            false => writeln!(s, "{}\n    c = color_{id}(c, q{q});", at(k)),
        };
    }
    s.push_str("    return c;\n}\n");
    s
}

type Source<'a> = Option<(bool, &'a str)>;

fn mask_of(e: &EffectUse) -> Option<Source<'_>> {
    let Some(m) = &e.mask else { return Some(None) };
    if m.shape.is_some() || m.feather > 0.0 {
        return None;
    }
    match (&m.layer, &m.texture) {
        (Some(layer), None) => Some(Some((true, layer.as_str()))),
        (None, Some(Value::Fixed(texture))) => Some(Some((false, texture.as_str()))),
        _ => None,
    }
}

fn fusable(e: &EffectUse) -> Option<(&'static Descriptor, Source<'_>)> {
    let pictured = |d: &Descriptor| d.params.iter().any(|p| p.kind.asset() && e.params.contains_key(&p.id));
    let mask = mask_of(e).filter(|_| e.enabled == Value::Fixed(true))?;
    let mixed = |d: &Descriptor| mask.is_some() && d.mask_mode == MaskMode::Mix && d.fuse == Some(Fuse::Uv);
    let d = descriptor(&e.effect)
        .filter(|d| !pictured(d) && !mixed(d) && d.fuse.is_some() && d.class() == Class::Light)?;
    Some((d, mask))
}

pub fn runs(effects: &[EffectUse]) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut k = 0;
    std::iter::from_fn(move || {
        while k < effects.len() {
            let (mut n, mut slots, mut seen) = (0, 0, [None; MASK_SLOTS]);
            while let Some((d, mask)) = effects.get(k + n).and_then(fusable) {
                slots += d.params.len() + MASK_SLOTS;
                let fresh = mask.filter(|m| !seen.contains(&Some(*m)));
                let room = seen.iter().position(Option::is_none);
                if slots > FX_SLOTS || fresh.is_some() && room.is_none() {
                    break;
                }
                if let (Some(m), Some(at)) = (fresh, room) {
                    seen[at] = Some(m);
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
