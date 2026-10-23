pub const BUS_NAME: &str = "io.github.wallpaper_rs.Daemon";
pub const OBJECT_PATH: &str = "/io/github/wallpaper_rs/Daemon";
pub const INTERFACE: &str = "io.github.wallpaper_rs.Daemon1";
pub const LIBRARY: &str = "io.github.wallpaper_rs.Library1";
pub const PLAYLISTS: &str = "io.github.wallpaper_rs.Playlists1";
pub const PROFILES: &str = "io.github.wallpaper_rs.Profiles1";
pub const PROVIDERS: &str = "io.github.wallpaper_rs.Providers1";
pub const REPOS: &str = "io.github.wallpaper_rs.Repos1";
pub const INPUTS: &str = "io.github.wallpaper_rs.Inputs1";
pub const SYNC: &str = "io.github.wallpaper_rs.Sync1";
pub const APP_ID: &str = "io.github.wallpaper_rs";
pub const APP_PATH: &str = "/io/github/wallpaper_rs";
pub const DEFAULT_OUTPUT: &str = "default";
pub const MAX_STREAM_EDGE: u32 = 16384;

pub fn uri_scheme(text: &str) -> Option<&str> {
    let (scheme, _) = text.split_once(':')?;
    let mut chars = scheme.chars();
    let first = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    (first && chars.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))).then_some(scheme)
}

pub mod error {
    pub const NOT_FOUND: &str = "io.github.wallpaper_rs.Error.NotFound";
    pub const EXISTS: &str = "io.github.wallpaper_rs.Error.Exists";
    pub const NEEDS_SECRET: &str = "io.github.wallpaper_rs.Error.NeedsSecret";
    pub const KEYRING_UNAVAILABLE: &str = "io.github.wallpaper_rs.Error.KeyringUnavailable";
    pub const STALE: &str = "io.github.wallpaper_rs.Error.Stale";
    pub const CONFLICT: &str = "io.github.wallpaper_rs.Error.Conflict";
    pub const NEWER: &str = "io.github.wallpaper_rs.Error.Newer";
    pub const UNCONFIRMED: &str = "io.github.wallpaper_rs.Error.Unconfirmed";
}
