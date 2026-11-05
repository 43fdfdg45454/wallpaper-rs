use std::path::Path;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use wprs_core::layer::Chosen;
use wprs_core::model::LayerShellState;
use wprs_core::{Config, LayerNamespace};

struct Started {
    option: Option<String>,
    environment: Option<String>,
    desktop: String,
    chosen: Chosen,
}

static STARTED: OnceLock<Started> = OnceLock::new();
static SHOWN: AtomicBool = AtomicBool::new(false);

pub(crate) fn valid(text: &str) -> Result<String, String> {
    LayerNamespace::given(text).map(|_| text.trim().to_owned())
}

pub(crate) fn choose(option: Option<&str>, config: &Path) -> String {
    let environment = std::env::var(LayerNamespace::ENV).ok();
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let file = Config::load(config).unwrap_or_default();
    let configured = file.surfaces.layer_namespace.as_ref();
    let chosen = LayerNamespace::choose(option, environment.as_deref(), configured, &desktop);
    if let Some(ignored) = &chosen.ignored {
        tracing::warn!("{ignored}");
    }
    tracing::info!(namespace = chosen.name, from = chosen.from.as_str(), "layer-shell namespace chosen");
    let name = chosen.name.clone();
    let option = option.map(str::to_owned);
    STARTED.get_or_init(|| Started { option, environment, desktop, chosen });
    name
}

pub(crate) fn shown() {
    SHOWN.store(true, Ordering::Relaxed);
}

pub(crate) fn state(config: &Config) -> LayerShellState {
    let Some(s) = STARTED.get().filter(|_| SHOWN.load(Ordering::Relaxed)) else {
        return LayerShellState::default();
    };
    let configured = config.surfaces.layer_namespace.as_ref();
    let now = LayerNamespace::choose(s.option.as_deref(), s.environment.as_deref(), configured, &s.desktop);
    LayerShellState {
        namespace: Some(s.chosen.name.clone()),
        from: s.chosen.from.as_str().into(),
        restart: now.name != s.chosen.name,
        ignored: s.chosen.ignored.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_option_is_checked_before_the_daemon_starts() {
        assert_eq!(valid(" desktop ").unwrap(), "desktop");
        assert_eq!(valid("auto").unwrap(), "auto");
        assert!(valid("a b").unwrap_err().contains("auto"));
        for blank in ["", "   "] {
            assert!(valid(blank).unwrap_err().contains("empty"), "{blank:?}");
        }
    }
}
