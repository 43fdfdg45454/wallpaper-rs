use crate::desk::Desk;
use crate::stacking::{Rations, clients, covered, notices, relevant};
use crate::x11::{Rect, X11};
use crate::{Cover, Event, Sink};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::sync::Arc;
use x11rb::connection::Connection as _;
use x11rb::errors::{ReplyError, ReplyOrIdError};
use x11rb::protocol::Event as Received;
use x11rb::protocol::randr::{self, ModeFlag, ModeInfo, MonitorInfo};
use x11rb::protocol::xproto::{ConnectionExt as _, Visibility};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Monitor {
    pub(crate) name: String,
    pub(crate) area: Rect,
    pub(crate) mm: (u32, u32),
    pub(crate) refresh: u32,
}

struct Screen {
    monitor: Monitor,
    desk: Arc<Desk>,
    covered: bool,
    hidden: bool,
}

struct Watch {
    x: Arc<X11>,
    sink: Sink,
    screens: Vec<Screen>,
    clients: HashSet<u32>,
    check: Option<u32>,
    root: (u16, u16),
    rations: Rations,
}

pub(crate) fn start(x: Arc<X11>, sink: Sink) -> anyhow::Result<()> {
    let setup = &x.conn.setup().roots[x.screen];
    let root = (setup.width_in_pixels, setup.height_in_pixels);
    let (screens, clients, check, rations) = Default::default();
    let mut watch = Watch { x, sink, screens, clients, check, root, rations };
    std::thread::Builder::new().name("wprs-x11".into()).spawn(move || {
        let Err(error) = watch.run();
        (watch.sink)(Event::Lost(error.to_string()))
    })?;
    Ok(())
}

impl Watch {
    fn run(&mut self) -> Result<Infallible, ReplyOrIdError> {
        let (mut monitors, mut stacking, mut manager) = (true, true, true);
        loop {
            if manager {
                self.manager()?;
            }
            if monitors {
                self.monitors()?;
            }
            if monitors || stacking || manager {
                self.stacking()?;
            }
            let (x, first) = (self.x.clone(), self.x.conn.wait_for_event()?);
            let more = std::iter::from_fn(|| x.conn.poll_for_event().transpose());
            let (a, mut exposed, mut resized) = (&x.atoms, HashSet::new(), false);
            (monitors, stacking, manager) = (false, false, false);
            for event in std::iter::once(Ok(first)).chain(more) {
                match event? {
                    Received::RandrScreenChangeNotify(_) | Received::RandrNotify(_) => monitors = true,
                    Received::ConfigureNotify(e) if e.window == x.root => {
                        monitors = true;
                        resized |=
                            std::mem::replace(&mut self.root, (e.width, e.height)) != (e.width, e.height);
                    }
                    Received::ConfigureNotify(e) => {
                        let theirs = self.screens.iter().all(|s| s.desk.window != e.window);
                        if !x.managed()
                            && theirs
                            && e.above_sibling == x11rb::NONE
                            && self.rations.lower_for(e.window)
                        {
                            self.screens.iter().try_for_each(|s| s.desk.lower())?;
                        }
                        stacking = true;
                    }
                    Received::VisibilityNotify(e) => {
                        let hidden = e.state == Visibility::FULLY_OBSCURED;
                        let screen = self.screens.iter_mut().find(|s| s.desk.window == e.window);
                        screen.into_iter().for_each(|s| s.hidden = hidden);
                        stacking = true;
                    }
                    Received::MapNotify(_) | Received::UnmapNotify(_) => stacking = true,
                    Received::DestroyNotify(e) => {
                        self.rations.forget(e.window);
                        manager |= self.check == Some(e.window);
                    }
                    Received::PropertyNotify(e)
                        if [a._NET_SUPPORTING_WM_CHECK, a._NET_SUPPORTED].contains(&e.atom) =>
                    {
                        manager |= e.window == x.root
                    }
                    Received::PropertyNotify(e) => stacking |= relevant(a, e.atom),
                    Received::Expose(e) if e.count == 0 => drop(exposed.insert(e.window)),
                    _ => {}
                }
            }
            let named = self.screens.iter().map(|s| (s.desk.window, s.monitor.name.clone()));
            notices(named, &exposed, resized).into_iter().for_each(|event| (self.sink)(event));
        }
    }

    fn manager(&mut self) -> Result<(), ReplyOrIdError> {
        let (managed, check) = self.x.manager()?;
        self.check = check;
        if self.x.managed.swap(managed, std::sync::atomic::Ordering::Relaxed) == managed {
            return Ok(());
        }
        tracing::info!(managed, "the window manager changed; replacing the desktop windows");
        for Screen { monitor, desk, .. } in &mut self.screens {
            *desk = Arc::new(Desk::new(&self.x, monitor.area)?);
            let window = desk.window(monitor, self.sink.clone());
            (self.sink)(Event::Output(monitor.name.clone(), window));
        }
        Ok(())
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
                    let (covered, hidden) = (false, false);
                    let screen = Screen { monitor: monitor.clone(), desk: desk.clone(), covered, hidden };
                    self.screens.push(screen);
                    desk
                }
            };
            let window = desk.window(&monitor, self.sink.clone());
            (self.sink)(Event::Output(monitor.name, window));
        }
        Ok(())
    }

    fn stacking(&mut self) -> Result<(), ReplyOrIdError> {
        let ours: Vec<u32> = self.screens.iter().map(|s| s.desk.window).collect();
        let mut clients = clients(&self.x, &mut self.clients)?;
        self.rations.restack(&self.x, &ours, &clients)?;
        clients.retain(|c| !ours.contains(&c.window));
        for screen in &mut self.screens {
            let now = covered(screen.monitor.area, &clients) || screen.hidden;
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
    let rates = rates(x).unwrap_or_default();
    Ok(list.iter().zip(names).enumerate().filter_map(|(i, (m, name))| monitor(i, name, m, &rates)).collect())
}

fn rates(x: &X11) -> Result<HashMap<u32, u32>, ReplyError> {
    let res = randr::get_screen_resources_current(&x.conn, x.root)?.reply()?;
    let modes: HashMap<u32, u32> = res.modes.iter().map(|m| (m.id, millihertz(m))).collect();
    let crtcs = res.crtcs.iter().map(|c| randr::get_crtc_info(&x.conn, *c, res.config_timestamp));
    let mut rates = HashMap::new();
    for crtc in crtcs.collect::<Result<Vec<_>, _>>()? {
        let crtc = crtc.reply()?;
        let rate = modes.get(&crtc.mode).copied().unwrap_or(0);
        rates.extend(crtc.outputs.iter().map(|o| (*o, rate)));
    }
    Ok(rates)
}

fn millihertz(m: &ModeInfo) -> u32 {
    let scan = |flag: ModeFlag| m.mode_flags.contains(flag);
    let lines = u64::from(m.vtotal) * if scan(ModeFlag::DOUBLE_SCAN) { 2 } else { 1 };
    let lines = if scan(ModeFlag::INTERLACE) { lines / 2 } else { lines };
    let rate = (u64::from(m.dot_clock) * 1000).checked_div(u64::from(m.htotal) * lines);
    rate.map_or(0, |r| r.min(u64::from(u32::MAX)) as u32)
}

fn monitor(
    index: usize,
    name: Option<String>,
    m: &MonitorInfo,
    rates: &HashMap<u32, u32>,
) -> Option<Monitor> {
    let area = Rect { x: m.x.into(), y: m.y.into(), width: m.width.into(), height: m.height.into() };
    let name = name.filter(|n| !n.is_empty()).unwrap_or_else(|| format!("X11-{index}"));
    let mm = (m.width_in_millimeters, m.height_in_millimeters);
    let refresh = m.outputs.iter().filter_map(|o| rates.get(o)).copied().max().unwrap_or(0);
    (area.width > 0 && area.height > 0).then_some(Monitor { name, area, mm, refresh })
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
        let area = Rect { x, y: 0, width: 640, height: 720 };
        Monitor { name: name.into(), area, mm: (340, 190), refresh: 0 }
    }

    #[test]
    fn monitors_keep_their_randr_names_and_skip_empty_areas() {
        let none = HashMap::new();
        assert_eq!(monitor(0, Some("DP-1".into()), &info(0, 640), &none), Some(named("DP-1", 0)));
        assert_eq!(monitor(1, None, &info(640, 640), &none), Some(named("X11-1", 640)));
        assert_eq!(monitor(2, Some(String::new()), &info(640, 640), &none), Some(named("X11-2", 640)));
        assert_eq!(monitor(0, Some("DP-2".into()), &info(0, 0), &none), None);
    }

    #[test]
    fn monitors_take_the_fastest_mode_of_their_outputs() {
        let mode = |dot_clock, htotal, vtotal, flags: u32| ModeInfo {
            dot_clock,
            htotal,
            vtotal,
            mode_flags: flags.into(),
            ..Default::default()
        };
        let interlaced = u32::from(ModeFlag::INTERLACE);
        assert_eq!(millihertz(&mode(148_500_000, 2200, 1125, 0)), 60_000);
        assert_eq!(millihertz(&mode(586_590_000, 2720, 1497, 0)), 144_060);
        assert_eq!(millihertz(&mode(74_250_000, 2200, 1125, interlaced)), 60_053);
        assert_eq!(millihertz(&mode(0, 0, 0, 0)), 0);
        let rates = HashMap::from([(7, 59_940), (8, 170_000)]);
        let fast = MonitorInfo { outputs: vec![7, 8], ..info(0, 640) };
        assert_eq!(monitor(0, Some("DP-1".into()), &fast, &rates).map(|m| m.refresh), Some(170_000));
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
