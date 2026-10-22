pub const BUS_NAME: &str = "io.github.wallpaper_rs.Daemon";
pub const OBJECT_PATH: &str = "/io/github/wallpaper_rs/Daemon";
pub const INTERFACE: &str = "io.github.wallpaper_rs.Daemon1";
pub const DEFAULT_OUTPUT: &str = "default";
pub const MAX_STREAM_EDGE: u32 = 16384;

pub fn uri_scheme(text: &str) -> Option<&str> {
    let (scheme, _) = text.split_once(':')?;
    let mut chars = scheme.chars();
    let first = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    (first && chars.all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))).then_some(scheme)
}
