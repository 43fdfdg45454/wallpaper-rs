use std::collections::HashMap;
use std::sync::LazyLock;

pub const CATALOGS: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));
static CATALOG: LazyLock<HashMap<String, String>> =
    LazyLock::new(|| catalog(&wprs_core::locale::languages()));

pub fn tr(text: &str) -> String {
    CATALOG.get(text).cloned().unwrap_or_else(|| text.to_owned())
}

pub fn catalog(languages: &[String]) -> HashMap<String, String> {
    let mut names = wprs_core::locale::names(languages);
    let found = names.find_map(|name| CATALOGS.iter().find(|(lang, _)| *lang == name));
    found.map(|(_, po)| parse(po)).unwrap_or_default()
}

pub fn parse(po: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for entry in po.split("\n\n") {
        if entry.lines().any(|l| l.starts_with("#,") && l.contains("fuzzy")) {
            continue;
        }
        let (mut parts, mut at) = ([String::new(), String::new()], None);
        for line in entry.lines().map(str::trim) {
            let quoted;
            (at, quoted) = match line.split_once(' ') {
                Some(("msgid", rest)) => (Some(0), rest),
                Some(("msgstr", rest)) => (Some(1), rest),
                _ if line.starts_with('"') => (at, line),
                _ => (None, ""),
            };
            if let (Some(i), Some(text)) = (at, quoted.strip_prefix('"').and_then(|q| q.strip_suffix('"'))) {
                parts[i].push_str(&unescape(text));
            }
        }
        let [id, text] = parts;
        if !id.is_empty() && !text.is_empty() {
            map.insert(id, text);
        }
    }
    map
}

fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        out.push(match c {
            '\\' => match chars.next() {
                Some('n') => '\n',
                Some('t') => '\t',
                Some(other) => other,
                None => break,
            },
            c => c,
        });
    }
    out
}
