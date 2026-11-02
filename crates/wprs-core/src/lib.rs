pub mod config;
pub mod damage;
pub mod dbus;
pub mod edit;
pub mod filter;
pub mod fit;
pub mod gpu;
mod keep;
pub mod layout;
pub mod locale;
pub mod markup;
pub mod message;
pub mod model;
pub mod motion;
mod options;
pub mod settings;
pub mod stash;
mod store;

pub use config::{Config, ConfigError, OutputConfig, Resolved};
pub use damage::{Ages, Region};
pub use edit::Edit;
pub use filter::{Filter, Grade, Look};
pub use fit::{Fit, UvRect};
pub use gpu::{GpuChoice, Render};
pub use layout::{Layout, Place, Screen};
pub use settings::{Library, Models, Pause, Playback, Repos, Transition, TransitionKind};
pub use store::{gpu_tier, stopped_effects};

pub const VERSION: &str = match option_env!("WPRS_VERSION") {
    Some(v) if !v.is_empty() => v,
    _ => concat!(env!("CARGO_PKG_VERSION"), "-local"),
};
