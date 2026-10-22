use crate::{Cache, Item, Media, Provider, Result, json};
use reqwest::{Client, RequestBuilder};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Immich {
    pub client: Client,
    pub server: String,
    pub api_key: String,
    pub album: String,
}

#[derive(Deserialize)]
struct Found {
    assets: Assets,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Assets {
    items: Vec<Asset>,
    next_page: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Asset {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    original_file_name: String,
}

impl Immich {
    pub fn new(server: &str, api_key: impl Into<String>, album: impl Into<String>) -> Result<Self> {
        let (client, server) = (crate::keyed_client()?, server.trim_end_matches('/').into());
        Ok(Self { client, server, api_key: api_key.into(), album: album.into() })
    }

    fn keyed(&self, request: RequestBuilder) -> RequestBuilder {
        request.header("x-api-key", &self.api_key)
    }

    fn item(&self, asset: Asset) -> Option<Item> {
        let (media, _) = Media::detect(&asset.original_file_name)?;
        let kind = match asset.kind.as_str() {
            "IMAGE" => Media::Image,
            "VIDEO" => Media::Video,
            _ => return None,
        };
        let url = format!("{}/api/assets/{}/original", self.server, asset.id);
        let (id, title) = (format!("immich-{}", asset.id), asset.original_file_name);
        (media == kind).then_some(Item { id, title, url, attribution: None, license: None, media })
    }
}

#[async_trait::async_trait]
impl Provider for Immich {
    async fn list(&self) -> Result<Vec<Item>> {
        let (mut items, mut page) = (Vec::new(), Some(1_u32));
        while let Some(number) = page {
            let body = json!({ "albumIds": [self.album], "page": number, "size": 1000 });
            let request = self.keyed(self.client.post(format!("{}/api/search/metadata", self.server)));
            let found: Found = json(request.json(&body)).await?;
            page = found.assets.next_page.and_then(|next| next.parse().ok()).filter(|&next| next > number);
            items.extend(found.assets.items.into_iter().filter_map(|asset| self.item(asset)));
        }
        Ok(items)
    }

    async fn fetch(&self, item: &Item, cache: &Cache) -> Result<PathBuf> {
        cache.download(item, self.keyed(self.client.get(&item.url))).await
    }
}
