use crate::Event;
use crate::cover::{covers, rationed};
use crate::x11::{Atoms, Rect, X11};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use x11rb::errors::{ConnectionError, ReplyOrIdError};
use x11rb::protocol::xproto::{
    AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask, GetPropertyReply, MapState,
};

const EVERY_DESKTOP: u32 = u32::MAX;
const LOWERED: usize = 3;
const LOWERED_WITHIN: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Client {
    pub(crate) window: u32,
    area: Rect,
    shown: bool,
    full: bool,
    desktop: bool,
    opaque: bool,
}

pub(crate) fn relevant(a: &Atoms, atom: u32) -> bool {
    [a._NET_CLIENT_LIST_STACKING, a._NET_CURRENT_DESKTOP, a._NET_WM_STATE, a._NET_WM_DESKTOP].contains(&atom)
}

pub(crate) fn clients(x: &X11, watched: &mut HashSet<u32>) -> Result<Vec<Client>, ReplyOrIdError> {
    let (c, a) = (&x.conn, &x.atoms);
    let windows = x.property(x.root, a._NET_CLIENT_LIST_STACKING)?;
    let current = x.property(x.root, a._NET_CURRENT_DESKTOP)?.first().copied();
    let listen = ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE);
    for window in windows.iter().filter(|w| !watched.contains(w)) {
        c.change_window_attributes(*window, &listen)?;
    }
    *watched = windows.iter().copied().collect();
    let asked = windows.iter().map(|&w| {
        let state = c.get_property(false, w, a._NET_WM_STATE, AtomEnum::ATOM, 0, 64)?;
        let desktop = c.get_property(false, w, a._NET_WM_DESKTOP, AtomEnum::CARDINAL, 0, 1)?;
        let kind = c.get_property(false, w, a._NET_WM_WINDOW_TYPE, AtomEnum::ATOM, 0, 16)?;
        let origin = c.translate_coordinates(w, x.root, 0, 0)?;
        Ok((w, state, desktop, kind, c.get_window_attributes(w)?, c.get_geometry(w)?, origin))
    });
    let asked = asked.collect::<Result<Vec<_>, ConnectionError>>()?;
    let marks = [
        a._NET_WM_STATE_FULLSCREEN,
        a._NET_WM_STATE_MAXIMIZED_VERT,
        a._NET_WM_STATE_MAXIMIZED_HORZ,
        a._NET_WM_STATE_HIDDEN,
    ];
    let atoms =
        |reply: GetPropertyReply| -> Vec<u32> { reply.value32().map(Iterator::collect).unwrap_or_default() };
    let read =
        asked.into_iter().filter_map(|(window, state, desktop, kind, attributes, geometry, origin)| {
            let states = atoms(state.reply().ok()?);
            let desktop = desktop.reply().ok()?.value32().and_then(|mut v| v.next());
            let desktop_window =
                kind.reply().ok().is_some_and(|k| atoms(k).contains(&a._NET_WM_WINDOW_TYPE_DESKTOP));
            let viewable = attributes.reply().ok()?.map_state == MapState::VIEWABLE;
            let (geometry, origin) = (geometry.reply().ok()?, origin.reply().ok()?);
            let (x, y, width, height) = (origin.dst_x, origin.dst_y, geometry.width, geometry.height);
            let area = Rect { x: x.into(), y: y.into(), width: width.into(), height: height.into() };
            let (full, hidden) = status(&states, marks);
            let shown = shown(viewable, hidden, desktop, current);
            Some(Client { window, area, shown, full, desktop: desktop_window, opaque: geometry.depth != 32 })
        });
    Ok(read.collect())
}

fn status(states: &[u32], [fullscreen, vertical, horizontal, hidden]: [u32; 4]) -> (bool, bool) {
    let has = |atom| states.contains(&atom);
    (has(fullscreen) || (has(vertical) && has(horizontal)), has(hidden))
}

fn shown(viewable: bool, hidden: bool, desktop: Option<u32>, current: Option<u32>) -> bool {
    viewable && !hidden && desktop.zip(current).is_none_or(|(d, c)| d == c || d == EVERY_DESKTOP)
}

pub(crate) fn covered(screen: Rect, clients: &[Client]) -> bool {
    let shown = || clients.iter().filter(|c| c.shown);
    let full = shown().any(|c| !c.desktop && c.full && screen.holds(c.area.center()));
    let tiles: Vec<Rect> = shown().filter(|c| !c.desktop).map(|c| c.area).collect();
    let icons = shown().filter(|c| c.desktop && c.opaque).any(|c| covers(screen, &[c.area]));
    full || icons || covers(screen, &tiles)
}

fn above(ours: &[u32], clients: &[Client]) -> Vec<(u32, u32)> {
    let mut pairs = Vec::new();
    for (i, icons) in
        clients.iter().enumerate().filter(|(_, c)| c.desktop && c.shown && !ours.contains(&c.window))
    {
        let higher = clients[i..].iter().filter(|c| ours.contains(&c.window));
        pairs.extend(higher.map(|ours| (ours.window, icons.window)));
    }
    pairs
}

#[derive(Default)]
pub(crate) struct Rations {
    fights: HashMap<u32, Vec<Instant>>,
    restacked: HashMap<(u32, u32), Vec<Instant>>,
}

impl Rations {
    pub(crate) fn lower_for(&mut self, window: u32) -> bool {
        let history = self.fights.entry(window).or_default();
        let allowed = rationed(history, Instant::now(), LOWERED, LOWERED_WITHIN);
        if !allowed && history.len() == LOWERED {
            tracing::info!("another program keeps its window at the bottom; the wallpaper stays above it");
            history.push(Instant::now());
        }
        allowed
    }

    pub(crate) fn forget(&mut self, window: u32) {
        self.fights.remove(&window);
    }

    pub(crate) fn restack(
        &mut self,
        x: &X11,
        ours: &[u32],
        clients: &[Client],
    ) -> Result<(), ReplyOrIdError> {
        for (mine, icons) in above(ours, clients) {
            let tries = self.restacked.entry((mine, icons)).or_default();
            if rationed(tries, Instant::now(), LOWERED, LOWERED_WITHIN) {
                tracing::debug!("keeping the wallpaper below the desktop icons");
                x.restack_below(mine, icons)?;
            }
        }
        self.restacked
            .retain(|(mine, icons), _| ours.contains(mine) && clients.iter().any(|c| c.window == *icons));
        Ok(())
    }
}

pub(crate) fn notices(
    screens: impl Iterator<Item = (u32, String)>,
    exposed: &HashSet<u32>,
    resized: bool,
) -> Vec<Event> {
    let mut events = Vec::new();
    for (window, name) in screens {
        if resized {
            events.push(Event::Backdrop(name.clone()));
        }
        if exposed.contains(&window) {
            events.push(Event::Exposed(name));
        }
    }
    events
}

#[cfg(test)]
mod tests;
