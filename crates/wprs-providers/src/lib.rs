mod cache;
mod daily;
mod fetch;
mod immich;
mod local;
mod wallhaven;
mod webdav;
mod wikimedia;

pub use cache::Cache;
pub use daily::{Apod, Bing};
pub use fetch::HttpFetch;
pub use immich::Immich;
pub use local::LocalFolder;
pub use wallhaven::Wallhaven;
pub use webdav::WebDav;
pub use wikimedia::WikimediaPotd;

use reqwest::redirect::{Action, Attempt, Policy};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;

const IMAGES: [&str; 6] = ["jpg", "jpeg", "png", "webp", "gif", "bmp"];
const VIDEOS: [&str; 7] = ["mp4", "webm", "mkv", "mov", "m4v", "avi", "ogv"];
const AGENT: &str = concat!("wallpaper-rs/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("http request failed: {0}")]
    Http(reqwest::Error),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid xml: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("invalid response: {0}")]
    Invalid(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error.without_url())
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(pub String);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(..)")
    }
}

impl<T: Into<String>> From<T> for Secret {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Media {
    Image,
    Video,
}

impl Media {
    pub fn detect(name: &str) -> Option<(Media, String)> {
        let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
        let media = match ext.as_str() {
            e if IMAGES.contains(&e) => Media::Image,
            e if VIDEOS.contains(&e) => Media::Video,
            _ => return None,
        };
        Some((media, ext))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub url: String,
    pub attribution: Option<String>,
    pub license: Option<String>,
    pub media: Media,
}

#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    async fn list(&self) -> Result<Vec<Item>>;
    async fn fetch(&self, item: &Item, cache: &Cache) -> Result<PathBuf>;
}

pub fn client() -> Result<reqwest::Client> {
    build(Policy::custom(|attempt| redirect(attempt, false)))
}

pub(crate) fn keyed_client() -> Result<reqwest::Client> {
    build(Policy::custom(|attempt| redirect(attempt, true)))
}

fn redirect(attempt: Attempt, same_host: bool) -> Action {
    let (url, previous) = (attempt.url(), attempt.previous());
    let downgrade = previous.last().is_some_and(|p| p.scheme() == "https") && url.scheme() != "https";
    let moved = same_host && previous.first().is_some_and(|first| first.origin() != url.origin());
    let hops = previous.len();
    match () {
        _ if downgrade || moved => attempt.stop(),
        _ if hops > 10 => attempt.error("too many redirects"),
        _ => attempt.follow(),
    }
}

fn build(redirects: Policy) -> Result<reqwest::Client> {
    let builder = reqwest::Client::builder().user_agent(AGENT);
    let builder = builder.connect_timeout(Duration::from_secs(15)).read_timeout(Duration::from_secs(30));
    Ok(builder.referer(false).redirect(redirects).build()?)
}

fn local(host: &str) -> bool {
    host == "localhost" || host.trim_matches(['[', ']']).parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

pub(crate) fn guarded(url: &str, allow_plain_http: bool) -> Result<()> {
    let parsed = reqwest::Url::parse(url).map_err(invalid)?;
    let safe = parsed.scheme() == "https" || allow_plain_http || parsed.host_str().is_some_and(local);
    safe.then_some(()).ok_or_else(|| invalid("refusing to send credentials over plain http"))
}

pub(crate) fn secure(url: &str) -> String {
    match reqwest::Url::parse(url) {
        Ok(mut parsed) if parsed.scheme() == "http" && !parsed.host_str().is_some_and(local) => {
            let _ = parsed.set_scheme("https");
            parsed.into()
        }
        _ => url.into(),
    }
}

pub(crate) async fn json<T: DeserializeOwned>(request: reqwest::RequestBuilder) -> Result<T> {
    Ok(request.send().await?.error_for_status()?.json().await?)
}

pub(crate) fn invalid(error: impl std::fmt::Display) -> Error {
    Error::Invalid(error.to_string())
}

pub(crate) fn path_of(url: &str) -> &str {
    url.split(['?', '#']).next().unwrap_or(url)
}

pub(crate) fn plain(html: &str) -> String {
    let mut parts = html.split('<');
    let head = parts.next().unwrap_or_default().to_string();
    let text = parts.fold(head, |text, tag| text + tag.split_once('>').map_or("", |(_, rest)| rest));
    let entities =
        [("&lt;", "<"), ("&gt;", ">"), ("&quot;", "\""), ("&#39;", "'"), ("&nbsp;", " "), ("&amp;", "&")];
    let text = entities.iter().fold(text, |text, (from, to)| text.replace(from, to));
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn utc_date(days_ago: u32) -> String {
    (chrono::Utc::now().date_naive() - chrono::Days::new(days_ago.into())).to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn upgrades_remote_http_only() {
        assert_eq!(super::secure("http://apod.nasa.gov/a.jpg"), "https://apod.nasa.gov/a.jpg");
        assert_eq!(super::secure("http://127.0.0.1:8080/a.jpg"), "http://127.0.0.1:8080/a.jpg");
        assert_eq!(super::secure("http://[::1]/a.jpg"), "http://[::1]/a.jpg");
        assert_eq!(super::secure("https://example.org/a.jpg"), "https://example.org/a.jpg");
    }

    #[test]
    fn credentials_need_tls_or_loopback() {
        assert!(super::guarded("http://example.org/dav", false).is_err());
        assert!(super::guarded("http://example.org/dav", true).is_ok());
        assert!(super::guarded("http://localhost:2283", false).is_ok());
        assert!(super::guarded("https://example.org/dav", false).is_ok());
    }

    #[test]
    fn secrets_stay_out_of_debug() {
        let text = format!("{:?}", super::Secret::from("hunter2"));
        assert!(!text.contains("hunter2"));
    }
}
