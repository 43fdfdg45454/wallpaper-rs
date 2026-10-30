use super::{DAY, clearable, field, interval, range, required, values};
use serde_json::{Value, json};
use wprs_core::model::{Field, FieldKind, ProviderKind, SecretField};

fn provider(
    kind: &str,
    hosts: &[&str],
    refresh: f64,
    secret: Option<(&str, bool)>,
    own: Vec<Field>,
) -> ProviderKind {
    let common = [
        interval("refresh", refresh, 900.0, true),
        range(field("cache_mb", FieldKind::Int, json!(1024)), 16.0, 65_536.0, Some(1.0)),
    ];
    let options = own.into_iter().chain(common).collect();
    let secret = secret.map(|(name, required)| SecretField { name: name.into(), required });
    ProviderKind { kind: kind.into(), hosts: hosts.iter().map(|h| h.to_string()).collect(), options, secret }
}

pub fn providers() -> Vec<ProviderKind> {
    let text = |key: &str, default: &str| field(key, FieldKind::Text, json!(default));
    let int = |key: &str, default: u32, min: f64, max: f64| {
        range(field(key, FieldKind::Int, json!(default)), min, max, Some(1.0))
    };
    let flag = |key: &str, default: bool| field(key, FieldKind::Bool, json!(default));
    let url = |key: &str| required(field(key, FieldKind::Url, Value::Null));
    let user = || required(field("user", FieldKind::Text, Value::Null));
    let sorting = ["date_added", "relevance", "random", "views", "favorites", "toplist"];
    vec![
        provider(
            "bing",
            &["www.bing.com"],
            DAY / 2.0,
            None,
            vec![text("market", "en-US"), flag("uhd", true), int("count", 8, 1.0, 8.0)],
        ),
        provider(
            "apod",
            &["api.nasa.gov", "apod.nasa.gov"],
            DAY / 2.0,
            Some(("api_key", false)),
            vec![int("days", 1, 1.0, 30.0), flag("videos", false)],
        ),
        provider(
            "wikimedia",
            &["commons.wikimedia.org", "upload.wikimedia.org"],
            DAY / 2.0,
            None,
            vec![int("days", 1, 1.0, 30.0), int("width", 3840, 640.0, 7680.0)],
        ),
        provider(
            "wallhaven",
            &["wallhaven.cc", "w.wallhaven.cc"],
            DAY,
            Some(("api_key", false)),
            vec![
                text("query", ""),
                text("categories", "111"),
                text("purity", "100"),
                values(field("sorting", FieldKind::Enum, json!("toplist")), &sorting),
                clearable(text("atleast", "")),
            ],
        ),
        provider(
            "webdav",
            &[],
            3600.0,
            Some(("password", true)),
            vec![url("url"), user(), flag("allow_plain_http", false)],
        ),
        provider(
            "nextcloud",
            &[],
            3600.0,
            Some(("password", true)),
            vec![url("server"), user(), text("folder", ""), flag("allow_plain_http", false)],
        ),
        provider(
            "immich",
            &[],
            3600.0,
            Some(("api_key", true)),
            vec![
                url("server"),
                required(field("album", FieldKind::Text, Value::Null)),
                flag("allow_plain_http", false),
            ],
        ),
    ]
}
