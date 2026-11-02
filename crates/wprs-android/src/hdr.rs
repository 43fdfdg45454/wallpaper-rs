use serde_json::{Value, json};
use std::sync::{Mutex, PoisonError};
use wprs_core::HdrChoice;
use wprs_core::hdr::{MAX_HEADROOM, UNKNOWN_HEADROOM};
use wprs_render::{RenderError, Renderer, Surface};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Screen {
    pub(crate) hdr: bool,
    pub(crate) peak_nits: f32,
    pub(crate) ratio: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Lit {
    active: bool,
    offered: bool,
    reason: &'static str,
    headroom: f32,
}

static CHOICE: Mutex<HdrChoice> = Mutex::new(HdrChoice::Auto);
static SCREEN: Mutex<Screen> = Mutex::new(Screen { hdr: false, peak_nits: 0.0, ratio: 0.0 });
static LIT: Mutex<Lit> = Mutex::new(Lit { active: false, offered: false, reason: "display", headroom: 1.0 });

fn get<T: Copy>(m: &Mutex<T>) -> T {
    *m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn put<T>(m: &Mutex<T>, value: T) {
    *m.lock().unwrap_or_else(PoisonError::into_inner) = value;
}

pub(crate) fn choose(choice: HdrChoice) {
    put(&CHOICE, choice);
}

pub(crate) fn screen(hdr: bool, peak_nits: f32, ratio: f32) {
    let fine = |v: f32| if v.is_finite() && v > 0.0 { v } else { 0.0 };
    put(
        &SCREEN,
        Screen { hdr, peak_nits: fine(peak_nits).min(100_000.0), ratio: fine(ratio).min(MAX_HEADROOM) },
    );
}

pub(crate) fn headroom(s: Screen) -> f32 {
    match (s.ratio, s.peak_nits) {
        (r, _) if r > 1.0 => r,
        (_, p) if p > 0.0 => (p / 203.0).clamp(1.0, MAX_HEADROOM),
        _ => UNKNOWN_HEADROOM,
    }
}

pub(crate) fn light(
    r: &Renderer,
    surface: &mut Surface,
    charging: bool,
    top: bool,
) -> Result<f32, RenderError> {
    let (want, reason) = get(&CHOICE).decide(surface.hdr_offered(), !charging, top);
    r.light(surface, want)?;
    let (active, offered) = (surface.hdr(), surface.hdr_offered());
    let reason = if want && !active { "display" } else { reason };
    let headroom = if active { headroom(get(&SCREEN)) } else { 1.0 };
    put(&LIT, Lit { active, offered, reason, headroom });
    Ok(headroom)
}

pub(crate) fn report() -> Value {
    let (lit, screen) = (get(&LIT), get(&SCREEN));
    json!({ "choice": get(&CHOICE).as_str(), "active": lit.active, "offered": lit.offered, "reason": lit.reason,
        "headroom": lit.headroom, "display": screen.hdr, "peak_nits": screen.peak_nits })
}
