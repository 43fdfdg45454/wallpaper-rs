use super::item::{Rect, linear, scale_of};
use bytemuck::{Pod, Zeroable};
use std::hash::{DefaultHasher, Hash, Hasher};
use wprs_scene::{Affine, Beam, Color, Frame, Lamp, MAX_LIGHTS, MAX_PROJECTIONS, Quality};

pub(crate) const FEW: usize = 8;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct Bulb {
    pub(crate) at: [f32; 4],
    pub(crate) color: [f32; 4],
    pub(crate) dir: [f32; 4],
    pub(crate) more: [f32; 4],
    pub(crate) air: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct Lights {
    pub(crate) head: [f32; 4],
    pub(crate) ambient: [f32; 4],
    pub(crate) sky: [f32; 4],
    pub(crate) bulbs: [Bulb; MAX_LIGHTS],
}

pub(crate) struct Glow {
    pub(crate) buffer: wgpu::Buffer,
    pub(crate) now: Lights,
    pub(crate) marks: [Mark; MAX_LIGHTS],
    pub(crate) moved: bool,
    pub(crate) baked: Option<(Affine, bool, usize)>,
    pub(crate) live: bool,
    pub(crate) seen: u64,
    pub(crate) slots: [usize; MAX_PROJECTIONS],
    pub(crate) gobos: Option<[super::Src; MAX_PROJECTIONS]>,
    written: Option<Lights>,
}

fn tone(c: Color, srgb: bool, by: f32) -> [f32; 4] {
    let conv = |v: f32| if srgb { v } else { linear(v) };
    [conv(c.0[0]) * by, conv(c.0[1]) * by, conv(c.0[2]) * by, 0.0]
}

pub(crate) fn bulb(lamp: &Lamp<'_>, screen: Affine, srgb: bool) -> (Bulb, Rect) {
    let (m, k) = (screen.then(lamp.world), scale_of(screen));
    let [x, y] = [m.0[4], m.0[5]];
    let [hx, hy, hz] = lamp.heading();
    let [a, b, ..] = lamp.world.0;
    let long = a.hypot(b).max(1e-6);
    let half = lamp.length * k * 0.5;
    let half_angle = |deg: f32| (deg * 0.5).to_radians().cos();
    let (outer, inner) = (half_angle(lamp.cone[1]), half_angle(lamp.cone[0]));
    let mut color = tone(lamp.color, srgb, lamp.intensity);
    color[3] = lamp.beam as u8 as f32;
    let radius = lamp.radius * k;
    let bulb = Bulb {
        at: [x, y, lamp.z * k, radius],
        color,
        dir: [hx, hy, hz, outer.min(inner - 1e-4)],
        more: [inner, a / long * half, b / long * half, 0.0],
        air: lamp.air.map_or([0.0; 4], |[density, samples, shafts]| [density, samples, 0.0, shafts]),
    };
    let ends = if lamp.beam == Beam::Tube { [bulb.more[1], bulb.more[2]] } else { [0.0; 2] };
    let rect = match lamp.beam {
        Beam::Directional => Rect { x: -1e7, y: -1e7, w: 2e7, h: 2e7 },
        _ => Rect::around([[x - ends[0], y - ends[1]], [x + ends[0], y + ends[1]]])
            .unwrap_or(Rect { x, y, w: 0.0, h: 0.0 })
            .grow(radius),
    };
    (bulb, rect)
}

pub(crate) type Mark = (usize, u64, Rect);

pub(crate) fn lights(
    frame: &Frame<'_>,
    (screen, srgb): (Affine, bool),
    cap: usize,
    (marks, slots): (&mut [Mark], &mut [usize; MAX_PROJECTIONS]),
) -> Lights {
    let mut out = Lights::default();
    let shown = frame.lamps.iter().take(cap.min(MAX_LIGHTS).min(marks.len()));
    let mut taken = 0;
    for (i, lamp) in shown.enumerate() {
        let (mut b, rect) = bulb(lamp, screen, srgb);
        if lamp.picture.is_some() && taken < MAX_PROJECTIONS {
            (slots[taken], taken, b.air[2]) = (i, taken + 1, (taken + 1) as f32);
        }
        out.bulbs[i] = b;
        let mut h = DefaultHasher::new();
        bytemuck::bytes_of(&b).hash(&mut h);
        marks[i] = (std::ptr::from_ref(lamp.layer) as usize, h.finish(), rect);
        out.head[0] = (i + 1) as f32;
    }
    out.head[1] = taken as f32;
    let ambient = frame.ambient.map_or([0.0; 4], |c| tone(c, srgb, 1.0));
    (out.ambient, out.sky) = (ambient, frame.sky.map_or(ambient, |c| tone(c, srgb, 1.0)));
    out
}

impl Glow {
    pub(crate) fn new(device: &wgpu::Device) -> Glow {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("wprs-scene lights"),
            size: size_of::<Lights>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let empty = (0, 0, Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 });
        Glow {
            buffer,
            now: Lights::default(),
            marks: [empty; MAX_LIGHTS],
            moved: false,
            baked: None,
            live: false,
            seen: 0,
            slots: [0; MAX_PROJECTIONS],
            gobos: None,
            written: None,
        }
    }

    pub(crate) fn update(
        &mut self,
        queue: &wgpu::Queue,
        frame: &Frame<'_>,
        screen: Affine,
        srgb: bool,
        cap: usize,
    ) {
        self.gobos = None;
        let key = (screen, srgb, cap);
        let keep = frame.quality == Quality::Q0 && self.baked == Some(key);
        self.baked = (frame.quality == Quality::Q0).then_some(key);
        if keep {
            self.moved = false;
            return;
        }
        let now = lights(frame, (screen, srgb), cap, (&mut self.marks, &mut self.slots));
        self.moved = self.written.is_some_and(|w| w != now);
        self.now = now;
        if self.written != Some(now) {
            queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&now));
            self.written = Some(now);
        }
    }

    pub(crate) fn count(&self) -> usize {
        self.now.head[0] as usize
    }

    pub(crate) fn pictures(&self) -> usize {
        self.now.head[1] as usize
    }

    pub(crate) fn airy(&self) -> bool {
        self.now.bulbs[..self.count()].iter().any(|b| b.air[0] > 0.0)
    }
}

impl crate::Renderer {
    pub(crate) fn light_cap(&self) -> usize {
        if self.tier() <= wprs_scene::Tier::T1 { FEW } else { MAX_LIGHTS }
    }
}
