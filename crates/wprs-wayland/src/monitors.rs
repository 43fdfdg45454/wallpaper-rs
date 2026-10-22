use crate::desk::Desk;
use crate::stacking::{clients, covered, relevant};
use crate::x11::{Rect, X11};
use crate::{Cover, Event, Sink};
use std::collections::HashSet;
use std::convert::Infallible;
use std::sync::Arc;
use x11rb::connection::Connection as _;
use x11rb::errors::ReplyOrIdError;
use x11rb::protocol::Event as Received;
use x11rb::protocol::randr::{self, MonitorInfo};
use x11rb::protocol::xproto::ConnectionExt as _;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Monitor {
    name: String,
    area: Rect,
    mm: (u32, u32),
}

struct Screen {
    monitor: Monitor,
    desk: Arc<Desk>,
    covered: bool,
}

struct Watch {
    x: Arc<X11>,
    sink: Sink,
    screens: Vec<Screen>,
    clients: HashSet<u32>,
}

pub(crate) fn start(x: Arc<X11>, sink: Sink) -> anyhow::Result<()> {
    let mut watch = Watch { x, sink, screens: Vec::new(), clients: HashSet::new() };
    std::thread::Builder::new().name("wprs-x11".into()).spawn(move || {
        let Err(error) = watch.run();
        (watch.sink)(Event::Lost(error.to_string()))
    })?;
    Ok(())
}

impl Watch {
    fn run(&mut self) -> Result<Infallible, ReplyOrIdError> {
        let (mut monitors, mut stacking) = (true, true);
        loop {
            if monitors {
                self.monitors()?;
            }
            if monitors || stacking {
                self.stacking()?;
            }
            let (x, first) = (self.x.clone(), self.x.conn.wait_for_event()?);
            let more = std::iter::from_fn(|| x.conn.poll_for_event().transpose());
            (monitors, stacking) = (false, false);
            for event in std::iter::once(Ok(first)).chain(more) {
                match event? {
                    Received::RandrScreenChangeNotify(_) | Received::RandrNotify(_) => monitors = true,
                    Received::ConfigureNotify(e) if e.window == x.root => monitors = true,
                    Received::ConfigureNotify(e) => {
                        let theirs = self.screens.iter().all(|s| s.desk.window != e.window);
                        if !x.managed && theirs && e.above_sibling == x11rb::NONE {
                            self.screens.iter().try_for_each(|s| s.desk.lower())?;
                        }
                        stacking = true;
                    }
                    Received::MapNotify(_) | Received::UnmapNotify(_) => stacking = true,
                    Received::PropertyNotify(e) => stacking |= relevant(&x.atoms, e.atom),
                    _ => {}
                }
            }
        }
    }

    fn monitors(&mut self) -> Result<(), ReplyOrIdError> {
        let old: Vec<Monitor> = self.screens.iter().map(|s| s.monitor.clone()).collect();
        let (gone, changed) = changes(&old, &monitors(&self.x)?);
        for name in gone {
            self.screens.retain(|s| s.monitor.name != name);
            (self.sink)(Event::Gone(name));
        }
        for monitor in changed {
            let desk = match self.screens.iter_mut().find(|s| s.monitor.name == monitor.name) {
                Some(screen) => {
                    screen.desk.place(monitor.area)?;
                    screen.monitor = monitor.clone();
                    screen.desk.clone()
                }
                None => {
                    let desk = Arc::new(Desk::new(&self.x, monitor.area)?);
                    tracing::info!(output = monitor.name, "desktop window created");
                    let screen = Screen { monitor: monitor.clone(), desk: desk.clone(), covered: false };
                    self.screens.push(screen);
                    desk
                }
            };
            let window = desk.window(&monitor.name, monitor.area, monitor.mm, self.sink.clone());
            (self.sink)(Event::Output(monitor.name, window));
        }
        Ok(())
    }

    fn stacking(&mut self) -> Result<(), ReplyOrIdError> {
        let ours: Vec<u32> = self.screens.iter().map(|s| s.desk.window).collect();
        let clients = clients(&self.x, &ours, &mut self.clients)?;
        for screen in &mut self.screens {
            let now = covered(screen.monitor.area, &clients);
            if std::mem::replace(&mut screen.covered, now) != now {
                (self.sink)(Event::Covered(screen.monitor.name.clone(), Cover::Toplevel, now));
            }
        }
        Ok(())
    }
}

fn monitors(x: &X11) -> Result<Vec<Monitor>, ReplyOrIdError> {
    let list = randr::get_monitors(&x.conn, x.root, true)?.reply()?.monitors;
    let names: Vec<_> = list.iter().map(|m| x.conn.get_atom_name(m.name)).collect::<Result<_, _>>()?;
    let names = names.into_iter().map(|n| n.reply().ok().map(|r| String::from_utf8_lossy(&r.name).into()));
    Ok(list.iter().zip(names).enumerate().filter_map(|(i, (m, name))| monitor(i, name, m)).collect())
}

fn monitor(index: usize, name: Option<String>, m: &MonitorInfo) -> Option<Monitor> {
    let area = Rect { x: m.x.into(), y: m.y.into(), width: m.width.into(), height: m.height.into() };
    let name = name.filter(|n| !n.is_empty()).unwrap_or_else(|| format!("X11-{index}"));
    let mm = (m.width_in_millimeters, m.height_in_millimeters);
    (area.width > 0 && area.height > 0).then_some(Monitor { name, area, mm })
}

fn changes(old: &[Monitor], new: &[Monitor]) -> (Vec<String>, Vec<Monitor>) {
    let gone = old.iter().filter(|o| new.iter().all(|n| n.name != o.name)).map(|o| o.name.clone());
    (gone.collect(), new.iter().filter(|n| !old.contains(n)).cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(x: i16, width: u16) -> MonitorInfo {
        let (height, width_in_millimeters, height_in_millimeters) = (720, 340, 190);
        MonitorInfo { x, width, height, width_in_millimeters, height_in_millimeters, ..Default::default() }
    }

    fn named(name: &str, x: i32) -> Monitor {
        Monitor { name: name.into(), area: Rect { x, y: 0, width: 640, height: 720 }, mm: (340, 190) }
    }

    #[test]
    fn monitors_keep_their_randr_names_and_skip_empty_areas() {
        assert_eq!(monitor(0, Some("DP-1".into()), &info(0, 640)), Some(named("DP-1", 0)));
        assert_eq!(monitor(1, None, &info(640, 640)), Some(named("X11-1", 640)));
        assert_eq!(monitor(2, Some(String::new()), &info(640, 640)), Some(named("X11-2", 640)));
        assert_eq!(monitor(0, Some("DP-2".into()), &info(0, 0)), None);
    }

    #[test]
    fn changes_list_removed_names_then_new_or_moved_monitors() {
        let old = [named("DP-1", 0), named("DP-2", 640)];
        assert_eq!(changes(&old, &old), (vec![], vec![]));
        let new = [named("DP-1", 640), named("HDMI-1", 0)];
        assert_eq!(changes(&old, &new), (vec!["DP-2".into()], new.to_vec()));
        assert_eq!(changes(&[], &old), (vec![], old.to_vec()));
        assert_eq!(changes(&old, &[]), (vec!["DP-1".into(), "DP-2".into()], vec![]));
    }
}
