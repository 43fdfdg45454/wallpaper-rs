use super::limb::{angle, mirrored, turn, wrap};
use super::{Puppet, Seat};
use crate::resolve::Context;
use crate::rigged::math::{IDENTITY, apply, invert, qmul};
use crate::{MeshLayer, RigControl};

impl Puppet {
    pub(crate) fn drive(&mut self, cx: &Context, m: &MeshLayer, set: &RigControl, seat: &Seat) {
        if set.bones.is_empty() {
            return;
        }
        for k in 0..self.order.len() {
            let n = self.order[k] as usize;
            let Some(b) = set.bones.get(&self.rig.joints[n].name) else { continue };
            let Puppet { rig, locals, worlds, .. } = self;
            let (rest, parent) = (rig.joints[n].rest, rig.joints[n].parent.map(|p| worlds[p as usize]));
            let l = &mut locals[n];
            match b.world {
                true => {
                    if let Some(p) = b.position {
                        let [x, y] = seat.boxed_at(p);
                        let back = parent.map_or(IDENTITY, |p| invert(&p));
                        l.t = apply(&back, [x, y, worlds[n][11]]);
                    }
                    if let Some(r) = b.rotation {
                        let now = angle([worlds[n][0], worlds[n][4]]);
                        turn(l, wrap(-r.to_radians() - now) * mirrored(parent.as_ref()));
                    }
                }
                false => {
                    if let Some(r) = b.rotation {
                        l.r = qmul(rest.r, crate::rigged::math::turn(r));
                    }
                    if let Some([x, y]) = b.position {
                        l.t = [rest.t[0] + x, rest.t[1] - y, rest.t[2]];
                    }
                }
            }
            if let Some([x, y]) = b.scale {
                l.s = [rest.s[0] * x, rest.s[1] * y, rest.s[2]];
            }
            if b.world {
                self.worlds(cx, m);
            }
        }
        self.worlds(cx, m);
    }

    pub(crate) fn push(&mut self, set: &RigControl, h: f32) {
        for (name, [x, y]) in &set.impulses {
            let Some(n) = self.rig.joint(name) else { continue };
            self.bodies.iter_mut().for_each(|b| b.push(n as u16, [*x, -*y], h));
            self.dirty = true;
        }
    }
}
