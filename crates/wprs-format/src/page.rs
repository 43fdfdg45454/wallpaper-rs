pub const API: &str = include_str!("page/api.js");
pub const MASK: &str = include_str!("page/mask.js");
pub const VOLUME: &str = include_str!("page/volume.js");
pub const INIT: &str = "__WPRS_INIT__";
pub const POST: &str = "__WPRS_POST__";

pub fn api(init: &serde_json::Value, post: &str) -> String {
    API.replace(INIT, &init.to_string()).replace(POST, post)
}

pub fn csp(own: &str, frames: &str, network: &[String]) -> String {
    let reach = format!("{own} {}", network.join(" "));
    [
        format!("default-src {own}"),
        format!("script-src {own} 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval'"),
        format!("style-src {own} 'unsafe-inline'"),
        format!("img-src {reach}"),
        format!("media-src {reach}"),
        format!("connect-src {reach}"),
        format!("font-src {own}"),
        format!("worker-src {own}"),
        format!("frame-src {frames}"),
        "object-src 'none'".into(),
        "form-action 'none'".into(),
        "base-uri 'none'".into(),
    ]
    .map(|d| d.trim_end().to_owned())
    .join("; ")
}

pub fn mime(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "json" | "gltf" => "application/json",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "webm" => "video/webm",
        "mp4" => "video/mp4",
        "mkv" => "video/x-matroska",
        "ogg" | "ogv" => "video/ogg",
        "opus" => "audio/ogg",
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "txt" | "glsl" | "vert" | "frag" | "wgsl" | "obj" | "mtl" => "text/plain",
        "csv" => "text/csv",
        "xml" => "application/xml",
        _ => "application/octet-stream",
    }
}

pub fn decoded(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

pub fn files(
    prop: &str,
    before: &[String],
    now: &[String],
    path: impl Fn(&str) -> Option<std::path::PathBuf>,
) -> Option<serde_json::Value> {
    let fresh: Vec<&String> = now.iter().filter(|t| !before.contains(t)).collect();
    let gone: Vec<&String> = before.iter().filter(|t| !now.contains(t)).collect();
    let same = |a: &str, b: &str| path(a).is_some_and(|p| path(b) == Some(p));
    let changed: Vec<(&String, &String)> =
        fresh.iter().filter_map(|t| gone.iter().find(|g| same(g, t)).map(|g| (*g, *t))).collect();
    let added: Vec<&String> = fresh.into_iter().filter(|t| !changed.iter().any(|c| c.1 == *t)).collect();
    let removed: Vec<&String> = gone.into_iter().filter(|t| !changed.iter().any(|c| c.0 == *t)).collect();
    let notice = serde_json::json!({"prop": prop, "added": added, "removed": removed, "changed": changed});
    (!added.is_empty() || !removed.is_empty() || !changed.is_empty()).then_some(notice)
}

pub fn token_name(path: &std::path::Path, hash: &str) -> String {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    let ext: String = ext.chars().filter(char::is_ascii_alphanumeric).take(8).collect();
    format!("{}.{ext}", &hash[..16.min(hash.len())])
}
