use crate::{Hitboxes, Runtime};

pub const MAX_TOUCHES: usize = 64;
pub const MAX_PRESSES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchKind {
    Click,
    Enter,
    Leave,
    Move,
    Down,
    Up,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Touch {
    pub layer: String,
    pub kind: TouchKind,
    pub local: [f32; 2],
    pub scene: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Press {
    pub at: Option<[f32; 2]>,
    pub buttons: u8,
}

#[derive(Debug, Default)]
pub(crate) struct Pointing {
    over: Option<(String, [f32; 2])>,
    held: Option<String>,
    buttons: u8,
    last: Option<[f32; 2]>,
    touches: Vec<Touch>,
    fired: Vec<String>,
}

impl Pointing {
    fn emit(&mut self, kind: TouchKind, layer: &str, local: [f32; 2], scene: [f32; 2]) {
        if self.touches.len() < MAX_TOUCHES {
            self.touches.push(Touch { layer: layer.to_owned(), kind, local, scene });
        }
        let word = match kind {
            TouchKind::Click => "click",
            TouchKind::Enter => "enter",
            TouchKind::Leave => "leave",
            _ => return,
        };
        self.fired.push(format!("{word}:{layer}"));
    }

    fn step(&mut self, boxes: &Hitboxes, view: [f32; 4], press: Press, (moves, clicks): (bool, bool)) {
        let scene = press.at.map(|[x, y]| [view[0] + x * view[2], view[1] + y * view[3]]);
        let at = scene.or(self.last).unwrap_or_default();
        let found = scene.and_then(|s| boxes.hit(s)).map(|(l, local)| (l.to_owned(), local));
        let name = |o: &Option<(String, [f32; 2])>| o.as_ref().map(|(l, _)| l.clone());
        if moves && name(&self.over) != name(&found) {
            if let Some((layer, local)) = self.over.take() {
                self.emit(TouchKind::Leave, &layer, local, at);
            }
            if let Some((layer, local)) = &found {
                self.emit(TouchKind::Enter, layer, *local, at);
            }
        }
        let moved = scene.is_some() && scene != self.last;
        let target = self.held.clone().or(name(&found));
        if let Some(layer) = target.filter(|_| moves && moved) {
            self.emit(TouchKind::Move, &layer, boxes.local(&layer, at), at);
        }
        let (pressed, released) = (press.buttons & !self.buttons, self.buttons & !press.buttons);
        if clicks && pressed != 0 {
            let layer = self.held.clone().or(name(&found));
            if let Some(layer) = layer {
                self.emit(TouchKind::Down, &layer, boxes.local(&layer, at), at);
                self.held = Some(layer);
            }
        }
        if clicks && released != 0 {
            let layer = self.held.clone().or(name(&found));
            if let Some(layer) = layer {
                self.emit(TouchKind::Up, &layer, boxes.local(&layer, at), at);
            }
            if press.buttons == 0 {
                let held = self.held.take();
                if let Some(layer) = held.filter(|h| Some(h) == name(&found).as_ref()) {
                    self.emit(TouchKind::Click, &layer, boxes.local(&layer, at), at);
                }
            }
        }
        if !clicks {
            self.held = None;
        }
        (self.over, self.buttons) = (found.filter(|_| moves), if clicks { press.buttons } else { 0 });
        self.last = scene.or(self.last);
    }
}

impl Runtime {
    pub fn point(
        &mut self,
        boxes: &Hitboxes,
        view: [f32; 4],
        presses: &[Press],
        allowed: (bool, bool),
    ) -> bool {
        let now = Press { at: self.pointer().pos, buttons: self.pointer().buttons };
        let p = &mut self.pointing;
        let before = p.touches.len();
        let now = allowed.0.then_some(&now);
        presses.iter().take(MAX_PRESSES).chain(now).for_each(|press| p.step(boxes, view, *press, allowed));
        let fired = std::mem::take(&mut p.fired);
        let touched = p.touches.len() > before;
        if self.scene.scripts.is_empty() {
            p.touches.clear();
        }
        for word in &fired {
            self.trigger_late(|t| t == word, 0.0);
        }
        touched || !fired.is_empty()
    }

    pub fn take_touches(&mut self) -> Vec<Touch> {
        std::mem::take(&mut self.pointing.touches)
    }
}
