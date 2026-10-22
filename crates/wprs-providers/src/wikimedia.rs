use crate::{Cache, Item, Media, Provider, Result, json, path_of, plain, utc_date};
use reqwest::Client;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct WikimediaPotd {
    pub client: Client,
    pub base: String,
    pub days: u32,
    pub width: u32,
}

#[derive(Deserialize)]
struct Expanded {
    expandtemplates: Wikitext,
}

#[derive(Deserialize)]
struct Wikitext {
    wikitext: String,
}

#[derive(Deserialize)]
struct Query {
    query: Pages,
}

#[derive(Deserialize)]
struct Pages {
    pages: Vec<Page>,
}

#[derive(Deserialize)]
struct Page {
    title: String,
    #[serde(default)]
    imageinfo: Vec<Info>,
}

#[derive(Deserialize)]
struct Info {
    url: String,
    thumburl: Option<String>,
    #[serde(default)]
    extmetadata: Metadata,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Metadata {
    artist: Option<Field>,
    license_short_name: Option<Field>,
    object_name: Option<Field>,
}

#[derive(Deserialize)]
struct Field {
    value: String,
}

impl WikimediaPotd {
    pub fn new(client: Client) -> Self {
        Self { client, base: "https://commons.wikimedia.org/w/api.php".into(), days: 1, width: 3840 }
    }

    async fn files(&self) -> Result<Vec<String>> {
        let potd: Vec<_> = (0..self.days.max(1)).map(|d| format!("{{{{Potd/{}}}}}", utc_date(d))).collect();
        let query = [("action", "expandtemplates"), ("prop", "wikitext"), ("text", &potd.join("\n"))];
        let expanded: Expanded = json(self.client.get(&self.base).query(&query).query(&FORMAT)).await?;
        let names = expanded.expandtemplates.wikitext.lines().map(str::trim).map(String::from);
        Ok(names.filter(|line| !line.is_empty() && !line.contains(['[', '{', '<', '|'])).collect())
    }
}

const FORMAT: [(&str, &str); 2] = [("format", "json"), ("formatversion", "2")];

fn item(page: Page) -> Option<Item> {
    let info = page.imageinfo.into_iter().next()?;
    let url = info.thumburl.unwrap_or(info.url);
    let (media, _) = Media::detect(path_of(&url))?;
    let name = page.title.trim_start_matches("File:");
    let meta = info.extmetadata;
    let title = meta.object_name.map(|field| plain(&field.value)).filter(|title| !title.is_empty());
    let title = title.unwrap_or_else(|| name.rsplit_once('.').map_or(name, |(stem, _)| stem).into());
    let attribution = meta.artist.map(|field| plain(&field.value));
    let license = meta.license_short_name.map(|field| field.value);
    Some(Item { id: format!("potd-{name}"), title, url, attribution, license, media })
}

#[async_trait::async_trait]
impl Provider for WikimediaPotd {
    async fn list(&self) -> Result<Vec<Item>> {
        let files = self.files().await?;
        if files.is_empty() {
            return Ok(Vec::new());
        }
        let titles = files.iter().map(|file| format!("File:{file}")).collect::<Vec<_>>().join("|");
        let info = [("action", "query"), ("prop", "imageinfo"), ("iiprop", "url|extmetadata")];
        let fields = ("iiextmetadatafilter", "Artist|LicenseShortName|ObjectName");
        let request = self.client.get(&self.base).query(&info).query(&[fields]).query(&FORMAT);
        let request = request.query(&[("iiurlwidth", self.width.to_string()), ("titles", titles)]);
        let found: Query = json(request).await?;
        Ok(found.query.pages.into_iter().filter_map(item).collect())
    }

    async fn fetch(&self, item: &Item, cache: &Cache) -> Result<PathBuf> {
        cache.download(item, self.client.get(crate::secure(&item.url))).await
    }
}
