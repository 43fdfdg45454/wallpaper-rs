use super::{REOPEN_WINDOW, REOPENED, Screen, State, turned};
use crate::Event;
use crate::cover::{allowed_at, rationed};
use crate::scale::effective;
use crate::window::{Display, Native, Window};
use std::time::Instant;

impl State {
    pub(crate) fn closed(&mut self, gone: impl Fn(&Screen) -> bool) {
        let Some(screen) = self.screens.iter().find(|s| gone(s)) else { return };
        let history = self.reopened.get(&screen.name).map_or(&[][..], Vec::as_slice);
        let later = allowed_at(history, REOPENED, REOPEN_WINDOW).filter(|at| *at > Instant::now());
        if let Some(wait) = later.map(|at| at.saturating_duration_since(Instant::now()).as_secs()) {
            tracing::info!(
                output = screen.name,
                wait,
                "the background surface was closed too often; waiting"
            );
        }
        self.reopen.push(screen.output.clone());
        self.remove(gone);
    }

    pub(crate) fn reopen(&mut self) {
        let mut waiting = Vec::new();
        for output in std::mem::take(&mut self.reopen) {
            let name = self
                .outputs
                .info(&output)
                .and_then(|i| i.name)
                .filter(|_| self.outputs.outputs().any(|o| o == output));
            let Some(name) = name.filter(|n| self.screens.iter().all(|s| s.name != *n)) else { continue };
            let history = self.reopened.entry(name.clone()).or_default();
            if !rationed(history, Instant::now(), REOPENED, REOPEN_WINDOW) {
                waiting.push(output);
                continue;
            }
            tracing::info!(output = name, "the compositor closed the background surface; creating it again");
            self.add(output);
        }
        self.reopen = waiting;
    }

    pub(crate) fn retry_at(&self) -> Option<Instant> {
        let names = self.reopen.iter().filter_map(|o| self.outputs.info(o)?.name);
        names.filter_map(|n| allowed_at(self.reopened.get(&n)?, REOPENED, REOPEN_WINDOW)).min()
    }

    pub(crate) fn remove(&mut self, gone: impl Fn(&Screen) -> bool) {
        let removed: Vec<_> = self.screens.extract_if(.., |s| gone(s)).collect();
        for screen in removed {
            match self.screens.iter().rev().find(|s| s.name == screen.name).map(|s| s.parts.layer.clone()) {
                Some(twin) => self.changed(|s| s.parts.layer == twin, |_| {}),
                None => (self.sink)(Event::Gone(screen.name)),
            }
        }
    }

    pub(crate) fn changed(&mut self, matches: impl Fn(&Screen) -> bool, change: impl FnOnce(&mut Screen)) {
        let Some(screen) = self.screens.iter_mut().find(|s| matches(s)) else { return };
        change(screen);
        let Some(logical) = screen.logical else { return };
        let info = self.outputs.info(&screen.output);
        let mode = info.as_ref().and_then(|i| i.modes.iter().find(|m| m.current));
        let window = Window {
            native: Native::Layer(screen.parts.clone(), self.qh.clone()),
            server: Display::Wayland(self.conn.clone()),
            sink: self.sink.clone(),
            name: screen.name.clone(),
            logical,
            scale: effective(screen.fractional, screen.integer, screen.parts.viewport.is_some()),
            refresh: screen.rate.current(mode.map_or(0, |m| m.refresh_rate.max(0) as u32)),
            position: info.as_ref().map_or((0, 0), |i| i.logical_position.unwrap_or(i.location)),
            mm: info.as_ref().map_or((0, 0), |i| turned(i, i.physical_size)),
            identity: info
                .as_ref()
                .map_or(String::new(), |i| format!("{} {}", i.make, i.model).trim().to_owned()),
        };
        (self.sink)(Event::Output(screen.name.clone(), window));
    }
}
