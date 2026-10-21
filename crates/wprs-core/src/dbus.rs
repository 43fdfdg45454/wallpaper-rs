//! Names of the daemon's D-Bus interface.
//!
//! These are provisional until the project name is final. Every component
//! must use these constants instead of literal strings.

/// Well-known bus name owned by the daemon.
pub const BUS_NAME: &str = "io.github.wallpaper_rs.Daemon";
/// Object path of the daemon object.
pub const OBJECT_PATH: &str = "/io/github/wallpaper_rs/Daemon";
/// Interface name, versioned.
pub const INTERFACE: &str = "io.github.wallpaper_rs.Daemon1";

/// Name used for an output when the caller does not know its connector name.
pub const DEFAULT_OUTPUT: &str = "default";

/// Largest stream edge the daemon accepts, in pixels.
pub const MAX_STREAM_EDGE: u32 = 16384;
