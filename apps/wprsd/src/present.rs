use crate::daemon::Daemon;
use std::collections::HashMap;
use std::sync::{Arc, mpsc};
use wprs_wayland::{Event, Session};
use zbus::fdo::DBusProxy;

const PLASMA: &str = "org.kde.plasmashell";
const SURFACE: &str = "surface";

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Backend {
    Auto,
    Streams,
    LayerShell,
    X11,
}

fn plan(backend: Backend, plasma: bool, wayland: bool, x11: bool) -> Option<(Backend, bool)> {
    match backend {
        Backend::Streams => None,
        Backend::Auto if plasma => None,
        Backend::Auto if wayland => Some((Backend::LayerShell, false)),
        Backend::Auto => x11.then_some((Backend::X11, false)),
        forced => Some((forced, true)),
    }
}

fn kde(desktop: &str) -> bool {
    desktop.split(':').any(|d| d.eq_ignore_ascii_case("KDE"))
}

pub async fn select(backend: Backend, bus: &DBusProxy<'_>) -> anyhow::Result<Option<Session>> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let plasma =
        backend == Backend::Auto && (kde(&desktop) || bus.name_has_owner(PLASMA.try_into()?).await?);
    let set = |name| std::env::var_os(name).is_some_and(|v| !v.is_empty());
    let x11 = set("DISPLAY") && !std::env::var("XDG_SESSION_TYPE").is_ok_and(|t| t == "wayland");
    let session = match plan(backend, plasma, set("WAYLAND_DISPLAY"), x11) {
        None => None,
        Some((chosen, required)) => {
            let (session, what) = match chosen {
                Backend::X11 => (wprs_wayland::connect_x11(), "X11 windows"),
                _ => (wprs_wayland::connect(), "layer-shell surfaces"),
            };
            match session {
                Err(e) if required => return Err(e.context(format!("cannot show {what}"))),
                session => session.inspect_err(|e| tracing::debug!("streams only: {e:#}")).ok(),
            }
        }
    };
    let backend = session.as_ref().map_or("streams", Session::kind);
    tracing::info!(backend, "presentation backend chosen");
    Ok(session)
}

pub fn start(session: Session, daemon: Arc<Daemon>) -> anyhow::Result<()> {
    let (tx, rx) = mpsc::channel();
    let frame = daemon.compositor.framer();
    session.start(move |event| match event {
        Event::Frame(output) => frame(&output),
        Event::Pointer(output, pos, buttons) => crate::inputs::surface_pointer(&output, pos, buttons),
        event => drop(tx.send(event)),
    })?;
    let follow = move || {
        let mut covers = HashMap::new();
        rx.into_iter().for_each(|event| daemon.surface_event(&mut covers, event))
    };
    std::thread::Builder::new().name("wprs-present".into()).spawn(follow)?;
    Ok(())
}

impl Daemon {
    fn surface_event(&self, covers: &mut HashMap<String, u8>, event: Event) {
        let result = match event {
            Event::Output(output, window) => {
                let (size, screen) = (window.size(), crate::edits::screen_of(&window));
                crate::inputs::window(&output, Some(window.clone()));
                self.compositor.attach(&output, window);
                {
                    let mut st = self.state();
                    let info = st.outputs.entry(output.clone()).or_default();
                    (info.size, info.screen) = (size, Some(screen));
                    info.claims.entry(SURFACE.into()).or_default();
                }
                self.configure_all();
                self.set_covered(&output, SURFACE, covers.get(&output).is_some_and(|c| *c != 0))
            }
            Event::Covered(output, cover, covered) => {
                let bits = covers.entry(output.clone()).or_default();
                *bits = (*bits & !(1 << cover as u8)) | (u8::from(covered) << cover as u8);
                self.set_covered(&output, SURFACE, *bits != 0)
            }
            Event::Gone(output) => {
                covers.remove(&output);
                crate::inputs::window(&output, None);
                self.state().outputs.remove(&output);
                self.compositor.remove(&output);
                tracing::info!(output, "output removed");
                self.configure_all();
                Ok(())
            }
            Event::Lost(reason) => {
                tracing::info!("the display server closed the connection ({reason}); exiting");
                std::process::exit(0)
            }
            Event::Frame(_) | Event::Pointer(..) => Ok(()),
        };
        result.unwrap_or_else(|e| tracing::warn!("cannot follow an output: {e:#}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plasma_keeps_streams_and_other_desktops_try_their_own_surfaces() {
        assert_eq!(plan(Backend::Auto, true, true, true), None);
        assert_eq!(plan(Backend::Auto, true, false, true), None);
        assert_eq!(plan(Backend::Auto, false, true, true), Some((Backend::LayerShell, false)));
        assert_eq!(plan(Backend::Auto, false, false, true), Some((Backend::X11, false)));
        assert_eq!(plan(Backend::Auto, false, false, false), None);
        assert_eq!(plan(Backend::Streams, false, true, true), None);
        assert_eq!(plan(Backend::LayerShell, true, false, false), Some((Backend::LayerShell, true)));
        assert_eq!(plan(Backend::X11, true, true, false), Some((Backend::X11, true)));
    }

    #[test]
    fn kde_sessions_are_recognised() {
        assert!(kde("KDE") && kde("kde") && kde("X-Generic:KDE"));
        assert!(!kde("") && !kde("sway") && !kde("GNOME:KDEish"));
    }
}
