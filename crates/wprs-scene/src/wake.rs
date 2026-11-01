use crate::fx::slots::live;
use crate::resolve::Context;
use crate::{Descriptor, MAX_PARAMS, Pace, Quality, Use};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wake {
    Frame,
    At(f64),
    Idle,
}

impl Wake {
    pub fn min(self, other: Wake) -> Wake {
        match (self, other) {
            (Wake::Frame, _) | (_, Wake::Frame) => Wake::Frame,
            (Wake::At(a), Wake::At(b)) => Wake::At(a.min(b)),
            (Wake::At(a), Wake::Idle) | (Wake::Idle, Wake::At(a)) => Wake::At(a),
            (Wake::Idle, Wake::Idle) => Wake::Idle,
        }
    }
}

pub(crate) fn wakes(d: &Descriptor, slots: &[[f32; 4]; MAX_PARAMS], cx: &Context, q: Quality) -> Wake {
    let yields =
        |p: &crate::ParamDef| p.yields_to.as_ref().and_then(|o| d.param(o)).is_some_and(|i| live(slots[i]));
    let gated =
        d.params.iter().zip(slots).filter(|(p, s)| live(**s) && !yields(p)).filter_map(|(p, _)| p.uses);
    let settle = d.state.as_ref().map_or(0.0, |s| s.settle);
    let wake = |u| match u {
        Use::Time => pace(d, slots, cx, q),
        Use::Cursor | Use::Clicks if cx.pointer.moving() || cx.calm[0] < settle => Wake::Frame,
        Use::Audio if !cx.audio.silent || cx.beat < 2.0 || cx.calm[1] < settle => Wake::Frame,
        _ => Wake::Idle,
    };
    d.uses.iter().copied().chain(gated).map(wake).fold(Wake::Idle, Wake::min)
}

fn pace(d: &Descriptor, slots: &[[f32; 4]; MAX_PARAMS], cx: &Context, q: Quality) -> Wake {
    let at = |id: &String| f64::from(d.param(id).map_or(0.0, |i| slots[i][0]));
    let t = f64::from(cx.t);
    match &d.pace {
        Some(Pace::Outside) if cx.pointer.inside => Wake::Idle,
        Some(Pace::Rate(id)) if at(id) > 0.0 => Wake::At(((t * at(id)).floor() + 1.0) / at(id) - t),
        Some(Pace::Steps([high, low])) => {
            let hz = f64::from(if q == Quality::Q0 { *low } else { *high }.max(1));
            Wake::At(((t * hz).floor() + 1.0) / hz - t)
        }
        Some(Pace::Sweep([speed, gap, phase])) => {
            let (v, cycle) = (at(speed), 1.0 + at(gap));
            match (t * v + at(phase)).rem_euclid(cycle) {
                x if x < 1.0 => Wake::Frame,
                x if v > 0.0 => Wake::At((cycle - x) / v),
                x if v < 0.0 => Wake::At((x - 1.0) / -v),
                _ => Wake::Idle,
            }
        }
        _ => Wake::Frame,
    }
}
