use crate::wire::Wire;
use wprs_scene::{AudioFrame, Course, Extent, Fired, LayerRead, PointerState, Touch, Values};

static NO_PROPS: Values = Values::new();
static IDLE: PointerState = PointerState {
    pos: None,
    smooth: [0.5; 2],
    velocity: [0.0; 2],
    buttons: 0,
    inside: false,
    presence: 0.0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Wallpaper,
    Lockscreen,
    Screensaver,
    Editor,
    Preview,
}

pub struct Frame<'a> {
    pub time: f64,
    pub dt: f32,
    pub wall: f64,
    pub zone: &'a str,
    pub props: &'a Values,
    pub actions: &'a [String],
    pub audio: Option<&'a AudioFrame>,
    pub pointer: &'a PointerState,
    pub view: [f32; 4],
    pub screen: [u32; 2],
    pub mode: Mode,
    pub fps: u32,
    pub quality: u8,
    pub timelines: &'a [Course],
    pub fired: &'a [Fired],
    pub reads: &'a [(String, LayerRead)],
    pub touches: &'a [Touch],
    pub sounds: &'a [String],
}

impl Frame<'static> {
    pub fn at(time: f64) -> Frame<'static> {
        Frame {
            time,
            dt: 0.0,
            wall: 0.0,
            zone: "",
            props: &NO_PROPS,
            actions: &[],
            audio: None,
            pointer: &IDLE,
            view: [0.0, 0.0, 1920.0, 1080.0],
            screen: [1920, 1080],
            mode: Mode::Wallpaper,
            fps: 60,
            quality: 3,
            timelines: &[],
            fired: &[],
            reads: &[],
            touches: &[],
            sounds: &[],
        }
    }
}

#[derive(Default)]
pub struct Encoder {
    pub(crate) buf: Vec<u8>,
    props: Option<Values>,
    zone: Option<String>,
    audio: Option<AudioFrame>,
    pub(crate) resend: bool,
}

impl Encoder {
    pub fn reset(&mut self) {
        (self.props, self.zone, self.audio) = (None, None, None);
    }

    pub fn encode(&mut self, f: &Frame) -> &[u8] {
        let w = &mut Wire(std::mem::take(&mut self.buf));
        w.0.clear();
        w.f64(f.time).f32(f.dt).f64(f.wall).u32(f.screen[0]).u32(f.screen[1]).f32s(&f.view);
        w.u32(f.fps).u8(f.quality).u8(f.mode as u8);
        let p = f.pointer;
        let [x, y] = p.pos.unwrap_or(p.smooth);
        w.f32(x).f32(y).u8(u8::from(p.inside)).u8(p.buttons);
        if w.flag(self.zone.as_deref() != Some(f.zone)) {
            w.str(f.zone);
            self.zone = Some(f.zone.to_owned());
        }
        if w.flag(self.props.as_ref() != Some(f.props)) {
            w.str(&serde_json::to_string(f.props).unwrap_or_default());
            self.props = Some(f.props.clone());
        }
        w.u16(f.actions.len());
        for a in f.actions {
            w.str(a);
        }
        let audio = f.audio.filter(|a| self.audio.as_ref() != Some(*a));
        w.flag(audio.is_some());
        if let Some(a) = audio {
            w.f32s(&a.left).f32s(&a.right).f32s(&a.average);
            w.f32(a.bass).f32(a.mid).f32(a.treble).f32(a.level).u8(u8::from(a.beat));
            self.audio = Some(a.clone());
        }
        w.u16(f.timelines.len());
        for c in f.timelines {
            w.str(&c.id).f32(c.time).f32(c.duration).u8(u8::from(c.playing)).f32(c.speed);
        }
        w.u16(f.fired.len());
        for e in f.fired {
            w.str(&e.timeline).opt(e.name.as_deref()).f32(e.t);
        }
        w.u16(f.reads.len());
        for (id, r) in f.reads {
            w.str(id).f32s(&r.position).f32s(&r.scale).f32s(&r.skew).f32s(&r.origin).f32s(&r.parallax);
            w.f32(r.rotation).f32(r.opacity).u8(u8::from(r.visible)).f32s(&r.color);
            match r.size {
                Some(Extent::Points(p)) => w.u8(1).f32(p),
                Some(Extent::Box(b)) => w.u8(2).f32s(&b),
                None => w.u8(0),
            };
            w.opt(r.text.as_deref());
        }
        w.u16(f.touches.len());
        for t in f.touches {
            w.str(&t.layer).u8(t.kind as u8).f32s(&t.local).f32s(&t.scene);
        }
        w.u16(f.sounds.len());
        for id in f.sounds {
            w.str(id);
        }
        w.flag(std::mem::take(&mut self.resend));
        self.buf = std::mem::take(&mut w.0);
        &self.buf
    }
}
