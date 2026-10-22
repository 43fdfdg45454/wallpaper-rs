use serde_json::{Value, json};
use wprs_core::model::{Field, FieldKind, MODEL, ProviderKind, QueryKey, Schema, SecretField};
use wprs_core::{Config, Fit};

const DAY: f64 = 86_400.0;
const FITS: [Fit; 5] = [Fit::Fill, Fit::Fit, Fit::Stretch, Fit::Center, Fit::Tile];
const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

fn field(key: &str, kind: FieldKind, default: Value) -> Field {
    let (min, max, step, values, required, clearable) = (None, None, None, Vec::new(), false, false);
    Field { key: key.into(), kind, default, min, max, step, values, required, clearable }
}

fn range(mut field: Field, min: f64, max: f64, step: Option<f64>) -> Field {
    (field.min, field.max, field.step) = (Some(min), Some(max), step);
    field
}

fn values(mut field: Field, values: &[&str]) -> Field {
    field.values = values.iter().map(|v| v.to_string()).collect();
    field
}

fn clearable(mut field: Field) -> Field {
    field.clearable = true;
    field
}

fn required(mut field: Field) -> Field {
    field.required = true;
    field
}

fn interval(key: &str, default: f64, min: f64, off: bool) -> Field {
    values(
        range(field(key, FieldKind::Interval, json!(default)), min, 30.0 * DAY, None),
        if off { &["0s"] } else { &[] },
    )
}

fn options() -> Vec<Field> {
    let defaults = serde_json::to_value(Config::default().options()).unwrap_or_default();
    let option = |key: &str, kind| field(key, kind, defaults.get(key).cloned().unwrap_or_default());
    let flag = |key: &str| option(key, FieldKind::Bool);
    vec![
        range(option("playback.volume", FieldKind::Float), 0.0, 1.0, Some(0.01)),
        flag("playback.muted"),
        range(option("playback.max_fps", FieldKind::Int), 1.0, 240.0, Some(1.0)),
        values(option("transition.kind", FieldKind::Enum), &["fade", "none"]),
        range(option("transition.duration_ms", FieldKind::Int), 0.0, 10_000.0, Some(1.0)),
        flag("pause.when_covered"),
        flag("pause.on_battery"),
        flag("pause.when_locked"),
        flag("library.watch"),
        range(option("library.thumb_cache_mb", FieldKind::Int), 64.0, 8192.0, Some(1.0)),
        flag("repos.auto_refresh"),
    ]
}

fn rules() -> Vec<Field> {
    vec![
        values(field("order", FieldKind::Enum, json!("sequential")), &["sequential", "shuffle"]),
        interval("interval", 1800.0, 10.0, true),
        clearable(range(field("at", FieldKind::Times, Value::Null), 0.0, 24.0, None)),
        clearable(values(field("days", FieldKind::Weekdays, Value::Null), &WEEKDAYS)),
        field("synced", FieldKind::Bool, json!(false)),
        field("resume", FieldKind::Bool, json!(true)),
        field("advance_while_paused", FieldKind::Bool, json!(false)),
        field("skip_videos_on_battery", FieldKind::Bool, json!(false)),
        clearable(range(field("transition_ms", FieldKind::Int, Value::Null), 0.0, 10_000.0, Some(1.0))),
    ]
}

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

pub fn schema(serial: u64) -> Schema {
    let auto = vec![
        clearable(field("outputs", FieldKind::List, Value::Null)),
        clearable(field("on_battery", FieldKind::Bool, Value::Null)),
        range(field("priority", FieldKind::Int, json!(0)), -1000.0, 1000.0, Some(1.0)),
    ];
    let fits: Vec<&str> = FITS.iter().map(|f| f.as_str()).collect();
    let preset = vec![
        values(field("fit", FieldKind::Enum, Value::Null), &fits),
        field("background", FieldKind::Color, Value::Null),
    ];
    let query = wprs_library::query_keys()
        .iter()
        .map(|(key, hint)| QueryKey { key: key.to_string(), hint: hint.to_string() });
    Schema {
        model: MODEL,
        serial,
        options: options(),
        rules: rules(),
        auto,
        preset,
        providers: providers(),
        query: query.collect(),
        thumb_sizes: crate::thumbs::SIZES.to_vec(),
        fits: FITS.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_is_described_with_its_default() {
        let schema = schema(1);
        let keys: Vec<String> = schema.options.iter().map(|f| f.key.clone()).collect();
        let known: Vec<String> = Config::default().options().into_keys().collect();
        assert_eq!(keys.iter().collect::<std::collections::BTreeSet<_>>(), known.iter().collect());
        assert!(schema.options.iter().all(|f| !f.default.is_null()));
        assert_eq!(schema.providers.len(), 7);
        assert!(schema.query.iter().any(|q| q.key == "sort"));
    }
}
