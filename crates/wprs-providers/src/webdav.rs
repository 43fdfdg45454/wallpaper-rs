use crate::{Cache, Item, Media, Provider, Result, Secret, invalid};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use reqwest::header::CONTENT_TYPE;
use reqwest::{Client, Method, RequestBuilder, Url};
use roxmltree::{Document, Node};
use std::path::PathBuf;

const DAV: &str = "DAV:";
const SEGMENT: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');
const PROPFIND: &str = r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:"><d:prop><d:resourcetype/></d:prop></d:propfind>"#;

#[derive(Debug, Clone)]
pub struct WebDav {
    pub client: Client,
    pub url: String,
    pub user: String,
    pub password: Secret,
    pub allow_plain_http: bool,
}

impl WebDav {
    pub fn new(url: impl Into<String>, user: impl Into<String>, password: impl Into<String>) -> Result<Self> {
        let client = crate::keyed_client()?;
        Ok(Self {
            client,
            url: url.into(),
            user: user.into(),
            password: Secret(password.into()),
            allow_plain_http: false,
        })
    }

    pub fn nextcloud(server: &str, user: &str, password: &str, folder: &str) -> Result<Self> {
        let segments = [user].into_iter().chain(folder.split('/')).filter(|segment| !segment.is_empty());
        let path: Vec<_> =
            segments.map(|segment| utf8_percent_encode(segment, SEGMENT).to_string()).collect();
        let url = format!("{}/remote.php/dav/files/{}/", server.trim_end_matches('/'), path.join("/"));
        Self::new(url, user, password)
    }

    fn auth(&self, request: RequestBuilder) -> RequestBuilder {
        request.basic_auth(&self.user, Some(&self.password.0))
    }
}

fn item(base: &Url, response: Node) -> Option<Item> {
    let tagged = |name: &str| response.descendants().find(|node| node.has_tag_name((DAV, name)));
    if tagged("collection").is_some() {
        return None;
    }
    let href = tagged("href")?.text()?.trim();
    let name =
        percent_decode_str(href.trim_end_matches('/').rsplit('/').next()?).decode_utf8_lossy().into_owned();
    let (media, _) = Media::detect(&name)?;
    let url = base.join(href).ok().filter(|url| url.origin() == base.origin())?;
    Some(Item {
        id: format!("webdav-{url}"),
        title: name,
        url: url.into(),
        attribution: None,
        license: None,
        media,
    })
}

#[async_trait::async_trait]
impl Provider for WebDav {
    async fn list(&self) -> Result<Vec<Item>> {
        let method = Method::from_bytes(b"PROPFIND").map_err(invalid)?;
        crate::guarded(&self.url, self.allow_plain_http)?;
        let request = self.auth(self.client.request(method, &self.url)).header("Depth", "1");
        let request = request.header(CONTENT_TYPE, "application/xml").body(PROPFIND);
        let body = request.send().await?.error_for_status()?.text().await?;
        let base = Url::parse(&self.url).map_err(invalid)?;
        let document = Document::parse(&body)?;
        let responses = document.descendants().filter(|node| node.has_tag_name((DAV, "response")));
        Ok(responses.filter_map(|response| item(&base, response)).collect())
    }

    async fn fetch(&self, item: &Item, cache: &Cache) -> Result<PathBuf> {
        crate::guarded(&item.url, self.allow_plain_http)?;
        cache.download(item, self.auth(self.client.get(&item.url))).await
    }
}
