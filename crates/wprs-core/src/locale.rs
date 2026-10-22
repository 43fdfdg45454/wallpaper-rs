use std::collections::HashMap;
use std::env;

pub fn languages() -> Vec<String> {
    let var = |key| env::var(key).ok().filter(|v| !v.is_empty());
    let locale = var("LC_ALL").or_else(|| var("LC_MESSAGES")).or_else(|| var("LANG")).unwrap_or_default();
    if locale.is_empty() || locale == "POSIX" || locale.split('.').next() == Some("C") {
        return Vec::new();
    }
    let preferred = var("LANGUAGE").unwrap_or_default();
    preferred.split(':').filter(|l| !l.is_empty()).map(String::from).chain([locale]).collect()
}

pub fn names(languages: &[String]) -> impl Iterator<Item = &str> {
    let names = languages.iter().map(|l| l.split(['.', '@']).next().unwrap_or_default());
    names.flat_map(|l| [l, l.split('_').next().unwrap_or_default()])
}

pub fn catalog(catalogs: &[(&str, &str)], languages: &[String]) -> HashMap<String, String> {
    let mut names = names(languages);
    let found = names.find_map(|name| catalogs.iter().find(|(lang, _)| *lang == name));
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
