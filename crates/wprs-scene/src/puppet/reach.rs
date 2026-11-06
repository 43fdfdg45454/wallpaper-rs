use super::Fig;
use super::limb::{angle, around, end, gap, length, mirrored, offset, pivot, range, tip, turn, wrap};
use super::{Puppet, Seat};
use crate::resolve::Context;
use crate::rigged::Rigged;
use crate::{Bend, Reach};

pub const IK_ROUNDS: usize = 10;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Goal {
    Point,
    Cursor,
    Bone(u16),
    Layer(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Chain {
    pub entry: usize,
    pub nodes: Vec<u16>,
    pub end: [f32; 3],
    pub goal: Goal,
    pub limits: Vec<Option<[f32; 2]>>,
    pub points: Vec<[f32; 2]>,
    pub turns: Vec<f32>,
    pub kept: Option<Box<Kept>>,
    pub ran: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Kept {
    from: Vec<[f32; 2]>,
    states: [(f32, f32); crate::MAX_CHAIN],
    target: [f32; 2],
    side: f32,
    turns: Vec<f32>,
}

pub(super) fn chains(rig: &Rigged, m: &Fig) -> Vec<Chain> {
    let resolve = |(entry, ik): (usize, &crate::IkChain)| {
        let nodes: Vec<u16> =
            ik.chain.iter().map(|b| rig.joint(b).map(|i| i as u16)).collect::<Option<_>>()?;
        let linked = nodes.windows(2).all(|w| rig.joints[w[1] as usize].parent == Some(w[0]));
        if !linked || !(2..=crate::MAX_CHAIN).contains(&nodes.len()) {
            return None;
        }
        let goal = match &ik.target {
            Reach::Point(_) => Goal::Point,
            Reach::Named(n) if n == "cursor" => Goal::Cursor,
            Reach::Named(n) => match n.split_once(':')? {
                ("bone", b) => Goal::Bone(rig.joint(b)? as u16),
                ("layer", id) => Goal::Layer(id.to_owned()),
                _ => return None,
            },
        };
        let limits = ik.chain.iter().map(|b| range(ik.limits.get(b).copied())).collect();
        let n = nodes.len();
        let end = tip(rig, *nodes.last()? as usize);
        let (points, turns) = (vec![[0.0; 2]; n + 1], vec![0.0; n]);
        Some(Chain { entry, nodes, end, goal, limits, points, turns, kept: None, ran: 0 })
    };
    m.rig.ik.iter().enumerate().filter_map(resolve).take(crate::MAX_CHAINS).collect()
}

fn clamp(c: &Chain, i: usize, now: f32, delta: f32, sign: f32) -> f32 {
    match c.limits[i] {
        Some([lo, hi]) => ((now + sign * delta).clamp(lo, hi) - now) * sign,
        None => delta,
    }
}

impl Puppet {
    pub(crate) fn reach(&mut self, cx: &Context, m: &Fig, seat: &Seat) {
        for k in 0..self.chains.len() {
            let ik = &m.rig.ik[self.chains[k].entry];
            let weight = ik.weight.get(cx).clamp(0.0, 1.0);
            let target = match &self.chains[k].goal {
                Goal::Point => match &ik.target {
                    Reach::Point(v) => Some(seat.boxed_at(v.get(cx).0)),
                    Reach::Named(_) => None,
                },
                Goal::Cursor => seat.cursor,
                Goal::Bone(b) => Some(pivot(&self.worlds[*b as usize])),
                Goal::Layer(id) => seat.layer(cx, id),
            };
            let Some(target) = target.filter(|_| weight > 0.0) else { continue };
            self.solve(k, target, ik.bend);
            let Puppet { chains, locals, worlds, rig, .. } = self;
            let c = &chains[k];
            for (i, &n) in c.nodes.iter().enumerate() {
                let parent = rig.joints[n as usize].parent.map(|p| &worlds[p as usize]);
                turn(&mut locals[n as usize], c.turns[i] * mirrored(parent) * weight);
            }
            self.worlds(cx, m);
        }
    }

    fn solve(&mut self, k: usize, target: [f32; 2], bend: Bend) {
        let Puppet { chains, locals, worlds, rig, .. } = self;
        let c = &mut chains[k];
        let n = c.nodes.len();
        for (i, &node) in c.nodes.iter().enumerate() {
            c.points[i] = pivot(&worlds[node as usize]);
        }
        c.points[n] = end(&worlds[c.nodes[n - 1] as usize], c.end);
        c.turns.iter_mut().for_each(|t| *t = 0.0);
        let state = |i: usize| {
            let node = c.nodes[i] as usize;
            let parent = rig.joints[node].parent.map(|p| &worlds[p as usize]);
            (offset(&locals[node], &rig.joints[node].rest), mirrored(parent))
        };
        let states: [(f32, f32); crate::MAX_CHAIN] =
            std::array::from_fn(|i| if i < n { state(i) } else { (0.0, 1.0) });
        let rotate = |c: &mut Chain, i: usize, delta: f32| {
            let (now, sign) = states[i];
            let delta = clamp(c, i, now + sign * c.turns[i], delta, sign);
            let center = c.points[i];
            c.points[i + 1..].iter_mut().for_each(|p| *p = around(*p, center, delta));
            c.turns[i] += delta;
        };
        let side = if bend == Bend::Clockwise { 1.0 } else { -1.0 };
        if let Some(k) = c.kept.as_deref().filter(|k| {
            k.from == c.points && k.states[..n] == states[..n] && k.target == target && k.side == side
        }) {
            c.turns.copy_from_slice(&k.turns);
            c.ran = 0;
            return;
        }
        let from = c.points.clone();
        let sided: [bool; crate::MAX_CHAIN] = std::array::from_fn(|i| {
            (1..n).contains(&i) && {
                let t =
                    wrap(angle(gap(c.points[i], c.points[i + 1])) - angle(gap(c.points[i - 1], c.points[i])));
                let (now, sign) = states[i];
                let room = c.limits[i].map(|[lo, hi]| {
                    let (a, b) = ((lo - now) * sign, (hi - now) * sign);
                    [a.min(b), a.max(b)]
                });
                super::arc::allows(t, room, side)
            }
        });
        let arced = n > 2 && (1..n).all(|i| sided[i]);
        c.ran = 0;
        let curl = |c: &mut Chain| {
            for (i, want) in super::arc::directions(&c.points, target, side).into_iter().enumerate() {
                rotate(c, i, wrap(want - angle(gap(c.points[i], c.points[i + 1]))));
            }
        };
        let seeded = arced && !super::arc::kept(&c.points, side);
        if seeded {
            curl(c);
        }
        let mut two = false;
        if n == 2 {
            let (l1, l2) = (length(gap(c.points[0], c.points[1])), length(gap(c.points[1], c.points[2])));
            let d = length(gap(c.points[0], target));
            if l1 > 1e-4 && l2 > 1e-4 && d > 1e-4 {
                let d = d.clamp((l1 - l2).abs(), l1 + l2);
                let cos = ((l1 * l1 + d * d - l2 * l2) / (2.0 * l1 * d)).clamp(-1.0, 1.0);
                let want = angle(gap(c.points[0], target)) + side * cos.acos();
                rotate(c, 0, wrap(want - angle(gap(c.points[0], c.points[1]))));
                let swing = angle(gap(c.points[1], target)) - angle(gap(c.points[1], c.points[2]));
                rotate(c, 1, wrap(swing));
                two = c.limits.iter().all(Option::is_none);
            }
        }
        let rounds = |c: &mut Chain| {
            for _ in 0..IK_ROUNDS {
                c.ran += 1;
                for i in (0..n).rev() {
                    let swing = wrap(angle(gap(c.points[i], target)) - angle(gap(c.points[i], c.points[n])));
                    let swing = match i {
                        _ if !sided[i] || n == 2 => swing,
                        _ => {
                            let before = gap(c.points[i - 1], c.points[i]);
                            super::arc::sided(gap(c.points[i], c.points[i + 1]), before, swing, side)
                        }
                    };
                    rotate(c, i, swing);
                }
                if length(gap(c.points[n], target)) < 0.01 {
                    return true;
                }
            }
            false
        };
        let (mut points, mut turns) = ([[0.0; 2]; crate::MAX_CHAIN + 1], [0.0; crate::MAX_CHAIN]);
        points[..=n].copy_from_slice(&c.points);
        turns[..n].copy_from_slice(&c.turns);
        if !two && !rounds(c) && arced && !seeded && super::arc::within(&points[..=n], target) {
            c.points.copy_from_slice(&points[..=n]);
            c.turns.copy_from_slice(&turns[..n]);
            curl(c);
            rounds(c);
        }
        let turns = c.turns.clone();
        c.kept = Some(Box::new(Kept { from, states, target, side, turns }));
    }
}
