use std::collections::HashMap;
use std::sync::LazyLock;

pub const CATALOGS: &[(&str, &str)] = include!(concat!(env!("OUT_DIR"), "/catalogs.rs"));
static CATALOG: LazyLock<HashMap<String, String>> =
    LazyLock::new(|| catalog(&wprs_core::locale::languages()));

pub fn tr(text: &str) -> String {
    CATALOG.get(text).cloned().unwrap_or_else(|| text.to_owned())
}

pub fn catalog(languages: &[String]) -> HashMap<String, String> {
    wprs_core::locale::catalog(CATALOGS, languages)
}
