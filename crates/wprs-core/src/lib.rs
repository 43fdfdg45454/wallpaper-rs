//! Core types shared by every wallpaper-rs component.

pub mod config;
pub mod dbus;
pub mod fit;

pub use config::{Config, ConfigError, OutputConfig};
pub use fit::{Fit, UvRect};
