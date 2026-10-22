use crate::{Cache, Item, Media, Provider, Result, json};
use reqwest::Client;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Wallhaven {
    pub client: Client,
    pub base: String,
    pub query: Option<String>,
    pub categories: String,
    pub purity: String,
    pub sorting: String,
    pub atleast: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Deserialize)]
struct Search {
    data: Vec<Wallpaper>,
}

#[derive(Deserialize)]
struct Wallpaper {
    id: String,
    url: String,
    path: String,
    resolution: String,
}

impl Wallhaven {
    pub fn new() -> Result<Self> {
        let (categories, purity, sorting) = ("111".into(), "100".into(), "toplist".into());
        let (client, base) = (crate::keyed_client()?, "https://wallhaven.cc/api/v1".into());
        Ok(Self { client, base, query: None, categories, purity, sorting, atleast: None, api_key: None })
    }
}

#[async_trait::async_trait]
impl Provider for Wallhaven {
    async fn list(&self) -> Result<Vec<Item>> {
        let mut query = vec![("categories", self.categories.as_str()), ("purity", &self.purity)];
        query.push(("sorting", &self.sorting));
        query.extend(self.query.as_deref().map(|q| ("q", q)));
        query.extend(self.atleast.as_deref().map(|size| ("atleast", size)));
        let request = self.client.get(format!("{}/search", self.base)).query(&query);
        let request = self.api_key.iter().fold(request, |request, key| request.header("X-API-Key", key));
        let search: Search = json(request).await?;
        let item = |paper: Wallpaper| Item {
            id: format!("wallhaven-{}", paper.id),
            title: format!("{} ({})", paper.id, paper.resolution),
            url: paper.path,
            attribution: Some(paper.url),
            license: None,
            media: Media::Image,
        };
        Ok(search.data.into_iter().map(item).collect())
    }

    async fn fetch(&self, item: &Item, cache: &Cache) -> Result<PathBuf> {
        cache.download(item, self.client.get(crate::secure(&item.url))).await
    }
}
