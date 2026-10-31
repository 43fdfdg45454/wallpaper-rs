use super::{Monitor, changes, monitors};
use crate::desk::{Desk, Felt, felt};
use crate::stacking::{Rations, clients, covered, notices, relevant};
use crate::x11::{Rect, X11};
use crate::{Cover, Event, Sink};
use std::collections::HashSet;
use std::convert::Infallible;
use std::sync::Arc;
use x11rb::connection::Connection as _;
use x11rb::errors::ReplyOrIdError;
use x11rb::protocol::Event as Received;
use x11rb::protocol::xproto::Visibility;

pub(super) struct Screen {
    monitor: Monitor,
    desk: Arc<Desk>,
    covered: bool,
    hidden: bool,
    buttons: u8,
}

pub(super) struct Watch {
    pub(super) x: Arc<X11>,
    pub(super) sink: Sink,
    pub(super) screens: Vec<Screen>,
    pub(super) clients: HashSet<u32>,
    pub(super) check: Option<u32>,
    pub(super) root: (u16, u16),
    pub(super) rations: Rations,
}

impl Watch {
    pub(super) fn run(&mut self) -> Result<Infallible, ReplyOrIdError> {
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
                    other => self.feel(felt(&other)),
                }
            }
            let named = self.screens.iter().map(|s| (s.desk.window, s.monitor.name.clone()));
            notices(named, &exposed, resized).into_iter().for_each(|event| (self.sink)(event));
        }
    }

    fn feel(&mut self, felt: Option<Felt>) {
        let window = match &felt {
            Some(Felt::At(w, _) | Felt::Button(w, ..) | Felt::Left(w)) => *w,
            None => return,
        };
        let Some(screen) = self.screens.iter_mut().find(|s| s.desk.window == window) else { return };
        let place = |[x, y]: [i16; 2]| {
            let Rect { width, height, .. } = screen.monitor.area;
            Some([f32::from(x) / width.max(1) as f32, f32::from(y) / height.max(1) as f32])
        };
        let pos = match felt {
            Some(Felt::At(_, at)) => place(at),
            Some(Felt::Button(_, at, bit, down)) => {
                screen.buttons = if down { screen.buttons | bit } else { screen.buttons & !bit };
                place(at)
            }
            _ => {
                screen.buttons = 0;
                None
            }
        };
        (self.sink)(Event::Pointer(screen.monitor.name.clone(), pos, screen.buttons));
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
                    let screen =
                        Screen { monitor: monitor.clone(), desk: desk.clone(), covered, hidden, buttons: 0 };
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
