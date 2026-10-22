use crate::daemon::Daemon;
use anyhow::Context;
use std::collections::HashMap;
use std::sync::{Arc, mpsc};
use wprs_wayland::{Event, Session};
use zbus::fdo::DBusProxy;

const PLASMA: &str = "org.kde.plasmashell";
const SURFACE: &str = "wayland";

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Backend {
    Auto,
    Streams,
    LayerShell,
}

fn plan(backend: Backend, plasma: bool, wayland: bool) -> Option<bool> {
    match backend {
        Backend::Streams => None,
        Backend::LayerShell => Some(true),
        Backend::Auto => (!plasma && wayland).then_some(false),
    }
}

fn kde(desktop: &str) -> bool {
    desktop.split(':').any(|d| d.eq_ignore_ascii_case("KDE"))
}

pub async fn select(backend: Backend, bus: &DBusProxy<'_>) -> anyhow::Result<Option<Session>> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let plasma =
        backend == Backend::Auto && (kde(&desktop) || bus.name_has_owner(PLASMA.try_into()?).await?);
    let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some_and(|d| !d.is_empty());
    let session = match plan(backend, plasma, wayland) {
        None => None,
        Some(true) => Some(wprs_wayland::connect().context("cannot show layer-shell surfaces")?),
        Some(false) => wprs_wayland::connect().inspect_err(|e| tracing::debug!("streams only: {e:#}")).ok(),
    };
    let backend = if session.is_some() { "layer-shell" } else { "streams" };
    tracing::info!(backend, "presentation backend chosen");
    Ok(session)
}

pub fn start(session: Session, daemon: Arc<Daemon>) -> anyhow::Result<()> {
    let (tx, rx) = mpsc::channel();
    let frame = daemon.compositor.framer();
    session.start(move |event| match event {
        Event::Frame(output) => frame(&output),
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
                let size = window.size();
                self.compositor.attach(&output, window);
                {
                    let mut st = self.state();
                    let info = st.outputs.entry(output.clone()).or_default();
                    info.size = size;
                    info.claims.entry(SURFACE.into()).or_default();
                }
                self.configure_output(&output);
                self.set_covered(&output, SURFACE, covers.get(&output).is_some_and(|c| *c != 0))
            }
            Event::Covered(output, cover, covered) => {
                let bits = covers.entry(output.clone()).or_default();
                *bits = (*bits & !(1 << cover as u8)) | (u8::from(covered) << cover as u8);
                self.set_covered(&output, SURFACE, *bits != 0)
            }
            Event::Gone(output) => {
                covers.remove(&output);
                self.state().outputs.remove(&output);
                self.compositor.remove(&output);
                tracing::info!(output, "output removed");
                Ok(())
            }
            Event::Lost(reason) => {
                tracing::info!("the Wayland compositor closed the connection ({reason}); exiting");
                std::process::exit(0)
            }
            Event::Frame(_) => Ok(()),
        };
        result.unwrap_or_else(|e| tracing::warn!("cannot follow an output: {e:#}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plasma_keeps_streams_and_other_wayland_desktops_try_layer_shell() {
        assert_eq!(plan(Backend::Auto, true, true), None);
        assert_eq!(plan(Backend::Auto, false, true), Some(false));
        assert_eq!(plan(Backend::Auto, false, false), None);
        assert_eq!(plan(Backend::Streams, false, true), None);
        assert_eq!(plan(Backend::LayerShell, true, false), Some(true));
    }

    #[test]
    fn kde_sessions_are_recognised() {
        assert!(kde("KDE") && kde("kde") && kde("X-Generic:KDE"));
        assert!(!kde("") && !kde("sway") && !kde("GNOME:KDEish"));
    }
}
