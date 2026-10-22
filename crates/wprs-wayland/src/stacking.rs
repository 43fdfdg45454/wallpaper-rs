use crate::x11::{Atoms, Rect, X11};
use std::collections::HashSet;
use x11rb::errors::{ConnectionError, ReplyOrIdError};
use x11rb::protocol::xproto::{AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask, MapState};

const EVERY_DESKTOP: u32 = u32::MAX;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Client {
    area: Rect,
    shown: bool,
    full: bool,
}

pub(crate) fn relevant(a: &Atoms, atom: u32) -> bool {
    [a._NET_CLIENT_LIST_STACKING, a._NET_CURRENT_DESKTOP, a._NET_WM_STATE, a._NET_WM_DESKTOP].contains(&atom)
}

pub(crate) fn clients(
    x: &X11,
    ours: &[u32],
    watched: &mut HashSet<u32>,
) -> Result<Vec<Client>, ReplyOrIdError> {
    let (c, a) = (&x.conn, &x.atoms);
    let mut windows = x.property(x.root, a._NET_CLIENT_LIST_STACKING)?;
    windows.retain(|w| !ours.contains(w));
    let current = x.property(x.root, a._NET_CURRENT_DESKTOP)?.first().copied();
    let listen = ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE);
    for window in windows.iter().filter(|w| !watched.contains(w)) {
        c.change_window_attributes(*window, &listen)?;
    }
    *watched = windows.iter().copied().collect();
    let asked = windows.iter().map(|&w| {
        let state = c.get_property(false, w, a._NET_WM_STATE, AtomEnum::ATOM, 0, 64)?;
        let desktop = c.get_property(false, w, a._NET_WM_DESKTOP, AtomEnum::CARDINAL, 0, 1)?;
        let origin = c.translate_coordinates(w, x.root, 0, 0)?;
        Ok((state, desktop, c.get_window_attributes(w)?, c.get_geometry(w)?, origin))
    });
    let asked = asked.collect::<Result<Vec<_>, ConnectionError>>()?;
    let marks = [
        a._NET_WM_STATE_FULLSCREEN,
        a._NET_WM_STATE_MAXIMIZED_VERT,
        a._NET_WM_STATE_MAXIMIZED_HORZ,
        a._NET_WM_STATE_HIDDEN,
    ];
    let read = asked.into_iter().filter_map(|(state, desktop, attributes, geometry, origin)| {
        let states: Vec<u32> = state.reply().ok()?.value32().map(Iterator::collect).unwrap_or_default();
        let desktop = desktop.reply().ok()?.value32().and_then(|mut v| v.next());
        let viewable = attributes.reply().ok()?.map_state == MapState::VIEWABLE;
        let (geometry, origin) = (geometry.reply().ok()?, origin.reply().ok()?);
        let (x, y, width, height) = (origin.dst_x, origin.dst_y, geometry.width, geometry.height);
        let area = Rect { x: x.into(), y: y.into(), width: width.into(), height: height.into() };
        let (full, hidden) = status(&states, marks);
        Some(Client { area, shown: shown(viewable, hidden, desktop, current), full })
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
    clients.iter().any(|c| c.shown && c.full && screen.holds(c.area.center()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKS: [u32; 4] = [1, 2, 3, 4];

    fn client(x: i32, width: u32, shown: bool, full: bool) -> Client {
        Client { area: Rect { x, y: 0, width, height: 720 }, shown, full }
    }

    #[test]
    fn fullscreen_or_fully_maximized_windows_are_full_and_hidden_ones_are_marked() {
        assert_eq!(status(&[9, 1], MARKS), (true, false));
        assert_eq!(status(&[2, 3], MARKS), (true, false));
        assert_eq!(status(&[2], MARKS), (false, false));
        assert_eq!(status(&[3, 4], MARKS), (false, true));
        assert_eq!(status(&[], MARKS), (false, false));
    }

    #[test]
    fn only_mapped_windows_on_the_current_desktop_are_shown() {
        assert!(shown(true, false, Some(1), Some(1)));
        assert!(shown(true, false, Some(EVERY_DESKTOP), Some(2)));
        assert!(shown(true, false, None, Some(2)) && shown(true, false, Some(3), None));
        assert!(!shown(true, false, Some(1), Some(2)));
        assert!(!shown(false, false, Some(1), Some(1)) && !shown(true, true, None, None));
    }

    #[test]
    fn a_monitor_is_covered_by_a_shown_full_window_centered_on_it() {
        let left = Rect { x: 0, y: 0, width: 640, height: 720 };
        let right = Rect { x: 640, ..left };
        let clients =
            [client(0, 640, true, true), client(640, 640, false, true), client(700, 300, true, false)];
        assert!(covered(left, &clients) && !covered(right, &clients));
        assert!(covered(right, &[client(400, 1000, true, true)]));
        assert!(!covered(left, &[client(400, 1000, true, true)]));
        assert!(!covered(left, &[]));
    }
}
