use crate::usage;
use anyhow::Result;
use clap::Args;
use wprs_providers::wallhaven::{self as values, CATEGORIES, PURITIES, option_hint};

#[derive(Args, Debug, Default)]
pub struct Filters {
    #[arg(
        long,
        help = "Search words; -word leaves a tag out, +word requires it, type:png or type:jpg picks a file type"
    )]
    query: Option<String>,
    #[arg(
        long,
        value_name = "NAMES",
        help = "Categories: general, anime and people separated by commas, or three digits like 101"
    )]
    categories: Option<String>,
    #[arg(
        long,
        value_name = "NAMES",
        help = "Purity: sfw, sketchy and nsfw separated by commas, or three digits like 110; sketchy and nsfw need the API key"
    )]
    purity: Option<String>,
    #[arg(long, help = "Sorting: date_added, relevance, random, views, favorites, toplist or hot")]
    sorting: Option<String>,
    #[arg(long, help = "Direction of the sorting: desc or asc")]
    order: Option<String>,
    #[arg(
        long,
        value_name = "RANGE",
        help = "Time range of the toplist sorting: 1d, 3d, 1w, 1M, 3M, 6M or 1y"
    )]
    top_range: Option<String>,
    #[arg(long, value_name = "WIDTHxHEIGHT", help = "Smallest resolution wanted")]
    atleast: Option<String>,
    #[arg(
        long,
        value_name = "SIZES",
        help = "Exact resolutions separated by commas, like 1920x1080,2560x1440"
    )]
    resolutions: Option<String>,
    #[arg(
        long,
        value_name = "RATIOS",
        help = "Aspect ratios separated by commas, like 16x9,16x10, or landscape and portrait"
    )]
    ratios: Option<String>,
    #[arg(
        long,
        value_name = "HEX",
        help = "Colors of the Wallhaven palette as hex without #, separated by commas, like 660000,ffffff"
    )]
    colors: Option<String>,
    #[arg(long, value_name = "ID", help = "Find pictures similar to the wallpaper with this id")]
    similar: Option<String>,
    #[arg(long, value_name = "NUMBER", help = "Find the pictures of the tag with this number")]
    tag: Option<String>,
    #[arg(long, value_name = "USER", help = "Only pictures uploaded by this user")]
    uploader: Option<String>,
    #[arg(
        long,
        value_name = "USER/ID",
        help = "Read a collection instead of searching; list them with the collections command"
    )]
    collection: Option<String>,
    #[arg(long, help = "First page to read (1 to 1000)")]
    page: Option<String>,
    #[arg(long, help = "Pages read on every sync (1 to 10); every page is one request")]
    pages: Option<String>,
    #[arg(long, help = "Most pictures kept on every sync (1 to 240)")]
    max_items: Option<String>,
    #[arg(long, value_name = "INTERVAL", help = "Sync every, like 12h; 0s for only when asked")]
    refresh: Option<String>,
    #[arg(long, help = "Disk space for the pictures, in MB (16 to 65536)")]
    cache_mb: Option<String>,
}

fn bits(flag: &str, text: &str, names: &[&str]) -> Result<String> {
    let text = text.trim();
    if text.len() == names.len() && text.chars().all(|c| c == '0' || c == '1') {
        return Ok(text.into());
    }
    let wanted: Vec<&str> = text.split(',').map(str::trim).filter(|name| !name.is_empty()).collect();
    let unknown = wanted.iter().find(|name| !names.contains(name));
    let hint =
        || usage(format!("--{flag} takes {} separated by commas, or digits like 101", names.join(", ")));
    unknown.map_or(Ok(()), |_| Err(hint()))?;
    Ok(names.iter().map(|name| if wanted.contains(name) { '1' } else { '0' }).collect())
}

fn choice(flag: &str, text: &str, allowed: &[&str]) -> Result<()> {
    let ok = allowed.contains(&text);
    ok.then_some(()).ok_or_else(|| usage(format!("--{flag} must be one of {}", allowed.join(", "))))
}

impl Filters {
    pub fn pairs(&self) -> Result<Vec<(&'static str, String)>> {
        let choices: [(&str, &Option<String>, &[&str]); 3] = [
            ("sorting", &self.sorting, &values::SORTINGS),
            ("order", &self.order, &values::ORDERS),
            ("top-range", &self.top_range, &values::RANGES),
        ];
        for (flag, value, allowed) in choices {
            value.iter().try_for_each(|text| choice(flag, text, allowed))?;
        }
        let given = [
            ("query", &self.query),
            ("atleast", &self.atleast),
            ("resolutions", &self.resolutions),
            ("ratios", &self.ratios),
            ("colors", &self.colors),
            ("similar", &self.similar),
            ("tag", &self.tag),
            ("uploader", &self.uploader),
            ("collection", &self.collection),
            ("sorting", &self.sorting),
            ("order", &self.order),
            ("top_range", &self.top_range),
            ("page", &self.page),
            ("pages", &self.pages),
            ("max_items", &self.max_items),
            ("refresh", &self.refresh),
            ("cache_mb", &self.cache_mb),
        ];
        let flags = [("categories", &self.categories, CATEGORIES), ("purity", &self.purity, PURITIES)];
        let mut pairs: Vec<(&'static str, String)> =
            given.iter().filter_map(|(key, value)| Some((*key, value.as_ref()?.trim().to_owned()))).collect();
        for (key, value, names) in flags {
            pairs.extend(
                value.as_ref().map(|text| bits(key, text, &names)).transpose()?.map(|bits| (key, bits)),
            );
        }
        for (key, value) in &pairs {
            if let Some(hint) = option_hint(key, value) {
                return Err(usage(format!("--{}: expected {hint}", key.replace('_', "-"))));
            }
        }
        Ok(pairs)
    }
}
