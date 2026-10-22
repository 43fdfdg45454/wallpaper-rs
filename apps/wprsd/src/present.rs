use wprs_wayland::Session;
use zbus::fdo::DBusProxy;

const PLASMA: &str = "org.kde.plasmashell";

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
    let sandboxed = crate::flatpak::app_id().is_some();
    let plasma = backend == Backend::Auto
        && (sandboxed || kde(&desktop) || bus.name_has_owner(PLASMA.try_into()?).await?);
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
