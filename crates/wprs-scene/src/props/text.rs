use super::Text;
use wprs_core::locale::names;

impl Text {
    pub fn pick(&self, languages: &[String]) -> String {
        let map = match self {
            Text::Plain(text) => return text.clone(),
            Text::Localized(map) => map,
        };
        let tags: Vec<(String, &String)> = map.iter().map(|(k, v)| (tag(k), v)).collect();
        let find = |name: String| {
            let exact = tags.iter().find(|(k, _)| *k == name);
            let base = || tags.iter().find(|(k, _)| k.split('-').next() == Some(name.as_str()));
            exact.or_else(base).map(|(_, v)| (*v).clone())
        };
        let found = names(languages).chain(["en"]).map(tag).find_map(find);
        found.or_else(|| map.values().next().cloned()).unwrap_or_default()
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Text::Plain(text) => text.is_empty(),
            Text::Localized(map) => map.is_empty(),
        }
    }
}

impl Default for Text {
    fn default() -> Self {
        Text::Plain(String::new())
    }
}

impl From<&str> for Text {
    fn from(text: &str) -> Self {
        Text::Plain(text.into())
    }
}

pub fn tag(name: &str) -> String {
    let tag = name.replace('_', "-").to_ascii_lowercase();
    match tag.as_str() {
        "zh-chs" | "zh-cn" | "zh-sg" => "zh-hans".into(),
        "zh-cht" | "zh-tw" | "zh-hk" | "zh-mo" => "zh-hant".into(),
        _ => tag,
    }
}
