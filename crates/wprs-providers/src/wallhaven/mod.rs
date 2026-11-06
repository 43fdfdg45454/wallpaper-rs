use crate::{Cache, Error, Item, Media, Provider, Result, Secret};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

const MAX_PAGES: u32 = 10;
const MAX_ITEMS: usize = 240;

mod check;
mod limit;
mod search;

pub use check::{
    CATEGORIES, ORDERS, PALETTE, PURITIES, RANGES, RATIOS, RESOLUTIONS, SORTINGS, hint as option_hint,
};
pub use search::Search;

#[derive(Debug, Clone)]
pub struct Wallhaven {
    pub client: Client,
    pub base: String,
    pub search: Search,
    pub api_key: Option<Secret>,
    pub backoff: Duration,
    pub window: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Collection {
    pub id: u64,
    pub label: String,
    pub count: u64,
    pub public: bool,
}

#[derive(Deserialize)]
struct Page {
    data: Vec<Wallpaper>,
    #[serde(default)]
    meta: Meta,
}

#[derive(Deserialize, Default)]
struct Meta {
    #[serde(default)]
    last_page: u32,
    seed: Option<String>,
}

#[derive(Deserialize)]
struct Wallpaper {
    id: String,
    url: String,
    path: String,
    resolution: String,
}

#[derive(Deserialize)]
struct Collections {
    data: Vec<Value>,
}

pub fn new_seed() -> String {
    let mut bytes = [0u8; 6];
    getrandom::fill(&mut bytes).expect("the system random source failed");
    bytes.iter().map(|b| char::from(b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"[usize::from(*b) % 36])).collect()
}

impl Wallhaven {
    pub fn new() -> Result<Self> {
        let (client, base) = (crate::keyed_client()?, "https://wallhaven.cc/api/v1".into());
        let (backoff, window) = (Duration::from_secs(2), Duration::from_secs(60));
        Ok(Self { client, base, search: Search::default(), api_key: None, backoff, window })
    }

    pub async fn collections(&self, owner: Option<&str>) -> Result<Vec<Collection>> {
        let path = match (owner, &self.api_key) {
            (Some(user), _) => format!("/collections/{user}"),
            (None, Some(_)) => "/collections".into(),
            (None, None) => return Err(Error::Setting("collections need a user name or an API key".into())),
        };
        let reply: Collections = self.get(&path, &[]).await?;
        let number = |entry: &Value, key: &str| entry[key].as_u64().unwrap_or_default();
        let public = |entry: &Value| entry["public"].as_bool().unwrap_or(number(entry, "public") == 1);
        let collection = |entry: Value| Collection {
            id: number(&entry, "id"),
            label: entry["label"].as_str().unwrap_or_default().chars().filter(|c| !c.is_control()).collect(),
            count: number(&entry, "count"),
            public: public(&entry),
        };
        Ok(reply.data.into_iter().map(collection).collect())
    }
}

#[async_trait::async_trait]
impl Provider for Wallhaven {
    async fn list(&self) -> Result<Vec<Item>> {
        let search = &self.search;
        search.validate().map_err(Error::Setting)?;
        if search.needs_key() && self.api_key.is_none() {
            return Err(Error::Setting("sketchy and nsfw pictures need an API key".into()));
        }
        let path = search.target().map_or("/search".into(), |(user, id)| format!("/collections/{user}/{id}"));
        let (mut seed, mut papers) = (search.seed.clone(), Vec::new());
        let (pages, wanted) = (search.pages.clamp(1, MAX_PAGES), search.max_items.clamp(1, MAX_ITEMS));
        for page in search.page..search.page.saturating_add(pages) {
            let reply: Page = self.get(&path, &search.params(page, seed.as_deref())).await?;
            seed = seed.or(reply.meta.seed);
            papers.extend(reply.data);
            if page >= reply.meta.last_page || papers.len() >= wanted {
                break;
            }
        }
        papers.truncate(wanted);
        let item = |paper: Wallpaper| Item {
            id: format!("wallhaven-{}", paper.id),
            title: format!("{} ({})", paper.id, paper.resolution),
            url: paper.path,
            attribution: Some(paper.url),
            license: None,
            media: Media::Image,
        };
        Ok(papers.into_iter().map(item).collect())
    }

    async fn fetch(&self, item: &Item, cache: &Cache) -> Result<PathBuf> {
        cache.download(item, self.client.get(crate::secure(&item.url))).await
    }
}
