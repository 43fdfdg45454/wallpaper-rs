use serde_json::{Value, json};

const OWN: &str = "wpak: wpak-user: data: blob:";

pub fn csp(network: &[String]) -> String {
    let hosts = network.join(" ");
    let reach = format!("{OWN} {hosts}");
    [
        format!("default-src {OWN}"),
        format!("script-src {OWN} 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval'"),
        format!("style-src {OWN} 'unsafe-inline'"),
        format!("img-src {reach}"),
        format!("media-src {reach}"),
        format!("connect-src {reach}"),
        format!("font-src {OWN}"),
        format!("worker-src {OWN}"),
        "frame-src wpak:".into(),
        "object-src 'none'".into(),
        "form-action 'none'".into(),
        "base-uri 'none'".into(),
    ]
    .map(|d| d.trim_end().to_owned())
    .join("; ")
}

fn escaped(origin: &str) -> String {
    origin.chars().flat_map(|c| if c.is_ascii_alphanumeric() { vec![c] } else { vec!['\\', c] }).collect()
}

pub fn filter(network: &[String]) -> String {
    let allow = |pattern: String| json!({"trigger": {"url-filter": pattern}, "action": {"type": "ignore-previous-rules"}});
    let own = ["^wpak:", "^wpak-user:", "^data:", "^blob:"].map(|p| allow(p.into()));
    let hosts = network.iter().map(|o| allow(format!("^{}/", escaped(o))));
    let block = json!({"trigger": {"url-filter": ".*"}, "action": {"type": "block"}});
    Value::Array(std::iter::once(block).chain(own).chain(hosts).collect()).to_string()
}

pub fn languages(real: &[String], hidden: bool) -> Vec<String> {
    let tag = |l: &String| l.split(['.', '@']).next().unwrap_or_default().replace('_', "-");
    let mut tags: Vec<String> =
        real.iter().map(tag).filter(|l| !l.is_empty() && l != "C" && l != "POSIX").collect();
    tags.dedup();
    match tags.is_empty() || hidden {
        true => vec!["en-US".into()],
        false => tags,
    }
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
