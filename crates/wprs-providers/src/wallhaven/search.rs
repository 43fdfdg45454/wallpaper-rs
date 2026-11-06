use super::check::items;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Search {
    pub query: Option<String>,
    pub categories: String,
    pub purity: String,
    pub sorting: String,
    pub order: String,
    pub top_range: String,
    pub atleast: Option<String>,
    pub resolutions: Vec<String>,
    pub ratios: Vec<String>,
    pub colors: Vec<String>,
    pub similar: Option<String>,
    pub tag: Option<String>,
    pub uploader: Option<String>,
    pub collection: Option<String>,
    pub page: u32,
    pub pages: u32,
    pub max_items: usize,
    pub seed: Option<String>,
}

impl Default for Search {
    fn default() -> Self {
        let text = |value: &str| value.to_owned();
        Self {
            query: None,
            categories: text("111"),
            purity: text("100"),
            sorting: text("toplist"),
            order: text("desc"),
            top_range: text("1M"),
            atleast: None,
            resolutions: Vec::new(),
            ratios: Vec::new(),
            colors: Vec::new(),
            similar: None,
            tag: None,
            uploader: None,
            collection: None,
            page: 1,
            pages: 1,
            max_items: 240,
            seed: None,
        }
    }
}

impl Search {
    pub fn from_options(get: impl Fn(&str) -> String) -> Self {
        let some = |key: &str| Some(get(key).trim().to_owned()).filter(|value| !value.is_empty());
        let list = |key: &str| items(&get(key)).map(str::to_lowercase).collect::<Vec<_>>();
        let number = |key: &str, default: u32| get(key).trim().parse().unwrap_or(default).max(1);
        let base = Self::default();
        let text = |key: &str, default: String| some(key).unwrap_or(default);
        Self {
            query: some("query"),
            categories: text("categories", base.categories),
            purity: text("purity", base.purity),
            sorting: text("sorting", base.sorting),
            order: text("order", base.order),
            top_range: text("top_range", base.top_range),
            atleast: some("atleast"),
            resolutions: list("resolutions"),
            ratios: list("ratios"),
            colors: list("colors").into_iter().map(|c| c.trim_start_matches('#').to_owned()).collect(),
            similar: some("similar"),
            tag: some("tag"),
            uploader: some("uploader"),
            collection: some("collection"),
            page: number("page", 1),
            pages: number("pages", 1),
            max_items: number("max_items", 240) as usize,
            seed: some("seed"),
        }
    }

    pub fn terms(&self) -> Option<String> {
        let like = self.similar.as_ref().map(|id| format!("like:{id}"));
        let tag = self.tag.as_ref().map(|id| format!("id:{id}"));
        let user = self.uploader.as_ref().map(|name| format!("@{name}"));
        let words = self.query.iter().cloned().chain(user).collect::<Vec<_>>().join(" ");
        like.or(tag).or(Some(words).filter(|words| !words.is_empty()))
    }

    pub fn params(&self, page: u32, seed: Option<&str>) -> Vec<(&'static str, String)> {
        let mut out = vec![("purity", self.purity.clone())];
        if self.target().is_none() {
            out.push(("categories", self.categories.clone()));
            out.push(("sorting", self.sorting.clone()));
            out.push(("order", self.order.clone()));
            out.extend((self.sorting == "toplist").then(|| ("topRange", self.top_range.clone())));
            out.extend(self.atleast.clone().map(|size| ("atleast", size)));
            for (key, list) in
                [("resolutions", &self.resolutions), ("ratios", &self.ratios), ("colors", &self.colors)]
            {
                out.extend((!list.is_empty()).then(|| (key, list.join(","))));
            }
            out.extend(self.terms().map(|terms| ("q", terms)));
            out.extend(seed.filter(|_| self.sorting == "random").map(|seed| ("seed", seed.to_owned())));
        }
        out.extend((page > 1).then(|| ("page", page.to_string())));
        out
    }

    pub fn target(&self) -> Option<(&str, &str)> {
        self.collection.as_deref()?.split_once('/')
    }

    pub fn validate(&self) -> Result<(), String> {
        let alone = self.similar.iter().chain(&self.tag).count();
        let mixed = alone > 1 || alone == 1 && (self.query.is_some() || self.uploader.is_some());
        let searching = alone > 0 || self.query.is_some() || self.uploader.is_some();
        if mixed {
            return Err("similar and tag searches cannot be combined with other search terms".into());
        }
        if searching && self.target().is_some() {
            return Err("a collection takes no search terms; clear them or the collection".into());
        }
        Ok(())
    }

    pub fn needs_key(&self) -> bool {
        self.purity.get(1..).is_some_and(|rest| rest.contains('1'))
    }
}
