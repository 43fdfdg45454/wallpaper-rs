use crate::x11::{Rect, X11};
use crate::{Event, Sink};
use std::collections::HashMap;
use std::sync::Arc;
use x11rb::connection::Connection as _;
use x11rb::errors::{ReplyError, ReplyOrIdError};
use x11rb::protocol::randr::{self, ModeFlag, ModeInfo, MonitorInfo};
use x11rb::protocol::xproto::ConnectionExt as _;

mod watch;

use watch::Watch;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Monitor {
    pub(crate) name: String,
    pub(crate) area: Rect,
    pub(crate) mm: (u32, u32),
    pub(crate) refresh: u32,
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
mod tests;
