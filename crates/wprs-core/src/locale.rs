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
