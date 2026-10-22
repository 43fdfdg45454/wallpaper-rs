use crate::{Cache, Item, Media, Provider, Result, json, path_of, plain, utc_date};
use reqwest::Client;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Bing {
    pub client: Client,
    pub base: String,
    pub market: String,
    pub uhd: bool,
    pub count: u8,
}

#[derive(Debug, Clone)]
pub struct Apod {
    pub client: Client,
    pub base: String,
    pub api_key: String,
    pub days: u32,
    pub videos: bool,
}

#[derive(Deserialize)]
struct Archive {
    images: Vec<Image>,
}

#[derive(Deserialize)]
struct Image {
    url: String,
    urlbase: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    copyright: String,
}

#[derive(Deserialize)]
struct Entry {
    date: String,
    title: String,
    media_type: String,
    url: Option<String>,
    hdurl: Option<String>,
    copyright: Option<String>,
}

impl Bing {
    pub fn new(client: Client, market: impl Into<String>, uhd: bool) -> Self {
        Self { client, base: "https://www.bing.com".into(), market: market.into(), uhd, count: 8 }
    }
}

impl Apod {
    pub fn new(client: Client, api_key: impl Into<String>) -> Self {
        let base = "https://api.nasa.gov/planetary/apod".into();
        Self { client, base, api_key: api_key.into(), days: 1, videos: false }
    }

    fn item(&self, entry: Entry) -> Option<Item> {
        let media = match entry.media_type.as_str() {
            "image" => Media::Image,
            "video" if self.videos => Media::Video,
            _ => return None,
        };
        let matches = |url: &String| Media::detect(path_of(url)).is_some_and(|(found, _)| found == media);
        let url = [entry.hdurl, entry.url].into_iter().flatten().find(matches)?;
        let (id, attribution) = (format!("apod-{}", entry.date), entry.copyright.as_deref().map(plain));
        Some(Item { id, title: entry.title, url, attribution, license: None, media })
    }
}

#[async_trait::async_trait]
impl Provider for Bing {
    async fn list(&self) -> Result<Vec<Item>> {
        let count = self.count.to_string();
        let query = [("format", "js"), ("idx", "0"), ("n", &count), ("mkt", &self.market)];
        let archive: Archive =
            json(self.client.get(format!("{}/HPImageArchive.aspx", self.base)).query(&query)).await?;
        let item = |image: Image| {
            let name = image.urlbase.rsplit('=').next().unwrap_or_default();
            let (id, path) = match self.uhd {
                true => (format!("bing-{name}_UHD"), format!("{}_UHD.jpg", image.urlbase)),
                false => (format!("bing-{name}"), image.url),
            };
            let title = if image.title.is_empty() { image.copyright.clone() } else { image.title };
            let url = format!("{}{path}", self.base);
            Item { id, title, url, attribution: Some(image.copyright), license: None, media: Media::Image }
        };
        Ok(archive.images.into_iter().map(item).collect())
    }

    async fn fetch(&self, item: &Item, cache: &Cache) -> Result<PathBuf> {
        cache.download(item, self.client.get(crate::secure(&item.url))).await
    }
}

#[async_trait::async_trait]
impl Provider for Apod {
    async fn list(&self) -> Result<Vec<Item>> {
        let start = utc_date(self.days.max(1));
        let query = [("api_key", self.api_key.as_str()), ("start_date", &start)];
        let entries: Vec<Entry> = json(self.client.get(&self.base).query(&query)).await?;
        Ok(entries.into_iter().filter_map(|entry| self.item(entry)).collect())
    }

    async fn fetch(&self, item: &Item, cache: &Cache) -> Result<PathBuf> {
        cache.download(item, self.client.get(crate::secure(&item.url))).await
    }
}
