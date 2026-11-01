use super::Sim;
use super::cloud::{Cloud, Particle};
use super::motion::{pcg, rand};
use super::pack::MAX_HATCH;
use super::spawn::{KEY, spawn};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub struct Event {
    pub pos: [f32; 2],
    pub vel: [f32; 2],
    pub color: [f32; 4],
    pub size: f32,
    pub rot: f32,
    pub id: u32,
    pub slot: u32,
    pub prev: [f32; 2],
    pub alive: u32,
    pub depth: f32,
}

impl Event {
    pub(super) fn of(p: &Particle, slot: usize, prev: [f32; 2]) -> Event {
        let (pos, vel, color, size, rot, id, depth) =
            (p.pos, p.vel, p.color, p.size, p.rot, p.id, p.depth[0]);
        let (slot, alive) = (slot as u32, u32::from(p.age < p.life));
        Event { pos, vel, color, size, rot, id, slot, prev, alive, depth }
    }
}

impl Cloud {
    pub(super) fn breed(&mut self, s: &Sim, host: Option<&Cloud>, points: &[[f32; 2]]) {
        match (s.mode[0], host) {
            (0, _) => (0..s.total()).for_each(|k| {
                let e = (0..7).find(|e| k < s.births[*e]).unwrap_or(7);
                let order = s.orders[e].wrapping_add(k - if e == 0 { 0 } else { s.births[e - 1] });
                let p = spawn(s, e, s.ids[1].wrapping_add(k), points, None, order);
                self.place(s, p);
            }),
            (1, Some(h)) => h.spawned.iter().for_each(|ev| hatch(self, s, ev, points)),
            (2, Some(h)) => h.died.iter().for_each(|ev| hatch(self, s, ev, points)),
            (3, Some(h)) => {
                h.mirror.iter().filter(|ev| ev.alive != 0).for_each(|ev| hatch(self, s, ev, points))
            }
            _ => {}
        }
    }
}

fn hatch(c: &mut Cloud, s: &Sim, ev: &Event, points: &[[f32; 2]]) {
    let seed = s.ids[0];
    if rand(seed, ev.id, KEY + 16 + s.hatch[0][1] as u32) >= s.hatch[0][0] {
        return;
    }
    let mut j = 0u32;
    for e in 0..(s.counts[1] as usize).clamp(1, 8) {
        let n = match s.mode[0] {
            3 => (s.rates[e] * s.clock[0] + rand(seed, ev.id ^ pcg(s.ids[2]), KEY + 32 + e as u32)) as u32,
            _ => s.births[e] - if e == 0 { 0 } else { s.births[e - 1] },
        };
        for _ in 0..n.min(MAX_HATCH) {
            if c.hatched >= s.mode[2] || c.full(s) {
                return;
            }
            c.hatched += 1;
            let id = pcg(ev.id ^ pcg(j ^ pcg(s.ids[2])));
            c.place(s, spawn(s, e, id, points, Some(ev), j));
            j += 1;
        }
    }
}
