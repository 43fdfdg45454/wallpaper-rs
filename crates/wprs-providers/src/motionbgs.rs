use crate::{Cache, Item, Media, Provider, Result, plain};
use reqwest::{Client, StatusCode};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct MotionBgs {
    pub client: Client,
    pub base: String,
    pub tag: Option<String>,
    pub uhd: bool,
    pub pages: u32,
}

struct Thumb<'a> {
    slug: &'a str,
    id: &'a str,
    title: &'a str,
    uhd: bool,
}

impl MotionBgs {
    pub fn new(client: Client) -> Self {
        Self { client, base: "https://motionbgs.com".into(), tag: None, uhd: true, pages: 1 }
    }

    fn page(&self, number: u32) -> String {
        let listing = self.tag.as_deref().map_or_else(|| "4k".into(), |tag| format!("tag:{tag}"));
        let number = if number > 1 { format!("{number}/") } else { String::new() };
        format!("{}/{listing}/{number}", self.base)
    }

    fn item(&self, thumb: Thumb) -> Item {
        let quality = if self.uhd && thumb.uhd { "4k" } else { "hd" };
        Item {
            id: format!("motionbgs-{}", thumb.id),
            title: plain(thumb.title),
            url: format!("{}/dl/{quality}/{}/", self.base, thumb.id),
            attribution: Some(format!("{}/{}", self.base, thumb.slug)),
            license: None,
            media: Media::Video,
        }
    }
}

fn between<'a>(text: &'a str, start: &str, end: char) -> Option<&'a str> {
    let rest = &text[text.find(start)? + start.len()..];
    Some(&rest[..rest.find(end)?])
}

fn thumbs(html: &str) -> impl Iterator<Item = Thumb<'_>> {
    let grid = html.find("class=tmb").map_or("", |at| &html[at..]);
    grid.split("<a title=").skip(1).filter_map(|link| {
        let link = &link[..link.find("</a>")?];
        let slug = between(link, "href=/", '>')?;
        let id = between(link, "/media/", '/')?;
        let title = between(link, "class=ttl>", '<')?.trim();
        let uhd = between(link, "class=frm>", '<').is_some_and(|badge| badge.trim() == "4K");
        let plain_slug = slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        let valid =
            plain_slug && !slug.is_empty() && !id.is_empty() && id.chars().all(|c| c.is_ascii_digit());
        valid.then_some(Thumb { slug, id, title, uhd })
    })
}

#[async_trait::async_trait]
impl Provider for MotionBgs {
    async fn list(&self) -> Result<Vec<Item>> {
        let (mut items, mut seen) = (Vec::new(), HashSet::new());
        for number in 1..=self.pages.max(1) {
            let response = self.client.get(self.page(number)).send().await?;
            if number > 1 && response.status() == StatusCode::NOT_FOUND {
                break;
            }
            let html = response.error_for_status()?.text().await?;
            let found: Vec<_> = thumbs(&html).filter(|thumb| seen.insert(thumb.id.to_owned())).collect();
            if found.is_empty() {
                break;
            }
            items.extend(found.into_iter().map(|thumb| self.item(thumb)));
        }
        Ok(items)
    }

    async fn fetch(&self, item: &Item, cache: &Cache) -> Result<PathBuf> {
        cache.download(item, self.client.get(crate::secure(&item.url))).await
    }
}
