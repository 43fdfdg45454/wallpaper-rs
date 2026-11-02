use super::{Fig, Puppet};
use crate::Loop;
use crate::rigged::Path;

impl Puppet {
    pub(crate) fn root(&mut self, m: &Fig) {
        self.next.travel = [0.0; 3];
        let Some(h) = self.heads.iter().find(|h| h.fade > 0.0 && h.weight.unwrap_or(1.0) > 0.0) else {
            return;
        };
        let (clip, map) = (&m.rig.clips[h.clip], &self.clips[h.clip]);
        let Some(motion) = map.motion.map(|i| &self.rig.motions[i]) else { return };
        let moves = |n: u16| motion.channels.iter().find(|c| c.node == n && c.path == Path::Translation);
        let Some(ch) = self.rig.order.iter().find_map(|n| moves(*n)) else { return };
        let len = map.length.max(1e-6);
        let t = h.t.max(0.0) as f32;
        let cycles = match clip.repeat {
            Loop::Repeat => h.laps as f32,
            _ => 0.0,
        };
        let local = crate::local(t, len, clip.repeat);
        let mut at = [0.0; 4];
        let mut sample = |s: f32| {
            ch.sample(clip.from + s, &mut at);
            [at[0], at[1], at[2]]
        };
        let (start, end, now) = (sample(0.0), sample(len), sample(local));
        let n = ch.node as usize;
        for k in [0, 2] {
            let moved = now[k] - start[k];
            self.next.travel[k] = moved + cycles * (end[k] - start[k]);
            self.locals[n].t[k] -= moved;
        }
    }
}
