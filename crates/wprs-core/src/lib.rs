pub mod config;
pub mod dbus;
pub mod edit;
pub mod fit;
pub mod layout;
pub mod locale;
pub mod markup;
pub mod model;
pub mod motion;
pub mod settings;
mod store;

pub use config::{Config, ConfigError, OutputConfig, Resolved};
pub use edit::Edit;
pub use fit::{Fit, UvRect};
pub use layout::{Layout, Place, Screen};
pub use settings::{Library, Models, Pause, Playback, Repos, Transition, TransitionKind};

pub const VERSION: &str = match option_env!("WPRS_VERSION") {
    Some(v) if !v.is_empty() => v,
    _ => concat!(env!("CARGO_PKG_VERSION"), "-local"),
};
