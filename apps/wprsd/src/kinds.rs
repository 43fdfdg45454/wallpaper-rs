use crate::error::{Result, invalid};
use reqwest::Url;
use serde_json::Value;
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::str::FromStr;
use wprs_core::model::{Field, FieldKind, ProviderKind};
use wprs_providers::{
    Apod, Bing, Immich, MotionBgs, Provider, Secret, Wallhaven, WebDav, WikimediaPotd, client,
};

pub type Options = BTreeMap<String, String>;
pub type Entry = (ProviderKind, Options, Options);

const SECRETS: [&str; 4] = ["password", "api_key", "token", "secret"];

pub fn kind_of(name: &str) -> Result<ProviderKind> {
    let known = crate::schema::providers().into_iter().find(|k| k.kind == name);
    known.ok_or_else(|| invalid(format!("unknown provider kind {name:?}; see GetSchema().providers")))
}

pub fn check_id(id: &str) -> Result<()> {
    let mut chars = id.chars();
    let first = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    let rest = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    if first && rest && id.len() <= 32 {
        return Ok(());
    }
    Err(invalid("a provider id is a lowercase letter and at most 31 lowercase letters, digits, - or _"))
}

pub fn local(url: &Url) -> bool {
    let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
    host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

pub fn parse_url(text: &str, query: bool) -> Result<Url> {
    let url = Url::parse(text).map_err(|_| invalid("not a valid URL"))?;
    let secret = !url.username().is_empty() || url.password().is_some() || url.fragment().is_some();
    if secret || (!query && url.query().is_some()) {
        return Err(invalid("put credentials in the secret, not in the URL"));
    }
    Ok(url)
}

fn server(text: &str, plain: bool) -> Result<()> {
    let url = parse_url(text, false)?;
    match url.scheme() {
        "https" => Ok(()),
        "http" if plain || local(&url) => Ok(()),
        "http" => {
            Err(invalid("refusing to send credentials over plain http; set allow_plain_http to allow it"))
        }
        _ => Err(invalid("use an https:// URL")),
    }
}

pub fn hosts(kind: &ProviderKind, options: &Options) -> Vec<String> {
    let url = ["url", "server"].iter().find_map(|key| Url::parse(options.get(*key)?).ok());
    let own = url.and_then(|url| url.host_str().map(String::from));
    if kind.hosts.is_empty() { own.into_iter().collect() } else { kind.hosts.clone() }
}

pub fn effective(kind: &ProviderKind, given: &Options) -> Result<Options> {
    for key in given.keys() {
        if SECRETS.contains(&key.as_str()) {
            return Err(invalid(format!("{key} is a secret; send it as the secret, never as an option")));
        }
        if !kind.options.iter().any(|field| &field.key == key) {
            return Err(invalid(format!("{} has no option {key}", kind.kind)));
        }
    }
    let mut options = Options::new();
    for field in &kind.options {
        let value = match given.get(&field.key) {
            Some(value) => checked(field, value.trim())?,
            None if field.required => {
                return Err(invalid(format!("{} needs the option {}", kind.kind, field.key)));
            }
            None => text(&field.default, field.kind),
        };
        options.insert(field.key.clone(), value);
    }
    let plain = options.get("allow_plain_http").is_some_and(|v| v == "true");
    ["url", "server"].iter().filter_map(|key| options.get(*key)).try_for_each(|url| server(url, plain))?;
    Ok(options)
}

fn text(value: &Value, kind: FieldKind) -> String {
    let seconds = value.as_f64().unwrap_or_default() as u64;
    match value {
        Value::Number(_) if kind == FieldKind::Interval => {
            wprs_schedule::format_interval(std::time::Duration::from_secs(seconds))
        }
        Value::Number(_) => seconds.to_string(),
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn checked(field: &Field, value: &str) -> Result<String> {
    let within = |n: f64| field.min.is_none_or(|min| n >= min) && field.max.is_none_or(|max| n <= max);
    let bits = || value.len() == 3 && value.chars().all(|c| c == '0' || c == '1');
    let size =
        || value.split_once('x').is_some_and(|(w, h)| w.parse::<u32>().is_ok() && h.parse::<u32>().is_ok());
    let word = || !value.is_empty() && value.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    let plain = value.len() <= 256 && !value.chars().any(char::is_control);
    let (ok, hint) = match (field.kind, field.key.as_str()) {
        (FieldKind::Bool, _) => (value == "true" || value == "false", "true or false".into()),
        (FieldKind::Int, _) => (value.parse::<i64>().is_ok_and(|n| within(n as f64)), range(field)),
        (FieldKind::Enum, _) => (field.values.iter().any(|v| v == value), field.values.join(", ")),
        (FieldKind::Interval, _) => {
            let seconds = wprs_schedule::parse_interval(value).map(|d| d.as_secs() as f64);
            let off = seconds == Some(0.0) && field.values.iter().any(|v| v == "0s");
            (off || seconds.is_some_and(within), "0s or an interval from 15m to 30d".into())
        }
        (FieldKind::Url, _) => (plain && !value.is_empty(), "a URL".into()),
        (_, "categories" | "purity") => (bits(), "three digits, each 0 or 1".into()),
        (_, "atleast") => (value.is_empty() || size(), "WIDTHxHEIGHT or nothing".into()),
        (_, "market" | "album") => (word(), "letters, digits and -".into()),
        (_, "tag") => (value.is_empty() || word(), "letters, digits and - or nothing".into()),
        (_, "folder") => (plain && !value.split('/').any(|s| s == ".."), "a folder path".into()),
        _ => (plain && (!field.required || !value.is_empty()), "a short text".into()),
    };
    if ok {
        Ok(value.to_owned())
    } else {
        Err(invalid(format!("invalid value for {}: expected {hint}", field.key)))
    }
}

fn range(field: &Field) -> String {
    format!("a whole number from {} to {}", field.min.unwrap_or_default(), field.max.unwrap_or_default())
}

pub fn entry(value: &toml::Value) -> std::result::Result<Entry, String> {
    let table = value.as_table().ok_or("the entry is not a table")?;
    let kind = table.get("kind").and_then(toml::Value::as_str).ok_or("the entry has no kind")?;
    let text = |(key, value): (&String, &toml::Value)| match value {
        toml::Value::String(text) => Ok((key.clone(), text.clone())),
        toml::Value::Boolean(flag) => Ok((key.clone(), flag.to_string())),
        toml::Value::Integer(number) => Ok((key.clone(), number.to_string())),
        _ => Err(format!("invalid value for {key}")),
    };
    let given = table.iter().filter(|(key, _)| *key != "kind").map(text).collect::<Result<Options, _>>()?;
    let known = kind_of(kind).map_err(|e| e.to_string())?;
    let options = effective(&known, &given).map_err(|e| e.to_string())?;
    Ok((known, given, options))
}

pub fn table(kind: &ProviderKind, given: &Options) -> toml::Value {
    let typed = |(key, value): (&String, &String)| {
        let field = kind.options.iter().find(|field| &field.key == key).map(|field| field.kind);
        let value = match (field, value.parse::<i64>()) {
            (Some(FieldKind::Bool), _) => toml::Value::Boolean(value == "true"),
            (Some(FieldKind::Int), Ok(number)) => toml::Value::Integer(number),
            _ => toml::Value::String(value.clone()),
        };
        (key.clone(), value)
    };
    let kind = ("kind".to_owned(), toml::Value::String(kind.kind.clone()));
    toml::Value::Table(std::iter::once(kind).chain(given.iter().map(typed)).collect())
}

fn number<T: FromStr + Default>(options: &Options, key: &str) -> T {
    options.get(key).and_then(|value| value.parse().ok()).unwrap_or_default()
}

pub fn build(kind: &str, o: &Options, secret: Option<&str>) -> wprs_providers::Result<Box<dyn Provider>> {
    let text = |key: &str| o.get(key).cloned().unwrap_or_default();
    let some = |key: &str| Some(text(key)).filter(|value| !value.is_empty());
    let (plain, key) = (text("allow_plain_http") == "true", secret.unwrap_or_default());
    Ok(match kind {
        "bing" => Box::new(Bing {
            count: number(o, "count"),
            ..Bing::new(client()?, text("market"), text("uhd") == "true")
        }),
        "apod" => {
            let apod = Apod::new(client()?, secret.unwrap_or("DEMO_KEY"));
            Box::new(Apod { days: number(o, "days"), videos: text("videos") == "true", ..apod })
        }
        "wikimedia" => Box::new(WikimediaPotd {
            days: number(o, "days"),
            width: number(o, "width"),
            ..WikimediaPotd::new(client()?)
        }),
        "wallhaven" => Box::new(Wallhaven {
            query: some("query"),
            categories: text("categories"),
            purity: text("purity"),
            sorting: text("sorting"),
            atleast: some("atleast"),
            api_key: secret.map(Secret::from),
            ..Wallhaven::new()?
        }),
        "motionbgs" => Box::new(MotionBgs {
            tag: some("tag").map(|tag| tag.to_ascii_lowercase()),
            uhd: text("uhd") == "true",
            pages: number(o, "pages"),
            ..MotionBgs::new(client()?)
        }),
        "webdav" => {
            Box::new(WebDav { allow_plain_http: plain, ..WebDav::new(text("url"), text("user"), key)? })
        }
        "nextcloud" => {
            let dav = WebDav::nextcloud(&text("server"), &text("user"), key, &text("folder"))?;
            Box::new(WebDav { allow_plain_http: plain, ..dav })
        }
        _ => {
            Box::new(Immich { allow_plain_http: plain, ..Immich::new(&text("server"), key, text("album"))? })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(pairs: &[(&str, &str)]) -> Options {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    fn refused(kind: &str, pairs: &[(&str, &str)]) -> String {
        effective(&kind_of(kind).unwrap(), &options(pairs)).unwrap_err().to_string()
    }

    #[test]
    fn options_are_validated_and_completed_with_defaults() {
        let bing = effective(&kind_of("bing").unwrap(), &options(&[("count", "3")])).unwrap();
        assert_eq!(
            bing,
            options(&[
                ("cache_mb", "1024"),
                ("count", "3"),
                ("market", "en-US"),
                ("refresh", "12h"),
                ("uhd", "true")
            ])
        );
        let immich = options(&[("server", "http://127.0.0.1:2283"), ("album", "0d6a-11"), ("refresh", "0s")]);
        assert_eq!(effective(&kind_of("immich").unwrap(), &immich).unwrap()["allow_plain_http"], "false");
        assert!(refused("bing", &[("count", "9")]).contains("from 1 to 8"));
        assert!(refused("bing", &[("refresh", "5m")]).contains("15m to 30d"));
        assert!(refused("bing", &[("password", "x")]).contains("is a secret"));
        assert!(refused("bing", &[("base", "x")]).contains("no option base"));
        assert!(refused("webdav", &[("url", "https://example.org/dav")]).contains("needs the option user"));
        assert!(refused("wallhaven", &[("purity", "123")]).contains("three digits"));
        assert!(refused("motionbgs", &[("tag", "../admin")]).contains("letters, digits and -"));
        assert!(refused("motionbgs", &[("pages", "11")]).contains("from 1 to 10"));
        let motion = effective(&kind_of("motionbgs").unwrap(), &options(&[("tag", "rain")])).unwrap();
        assert_eq!(
            (motion["tag"].as_str(), motion["uhd"].as_str(), motion["pages"].as_str()),
            ("rain", "true", "1")
        );
        for url in
            ["https://me:pw@example.org/dav", "https://example.org/dav?token=1", "https://example.org/#x"]
        {
            assert!(refused("webdav", &[("url", url), ("user", "me")]).contains("not in the URL"), "{url}");
        }
        assert!(
            refused("webdav", &[("url", "http://example.org/dav"), ("user", "me")]).contains("plain http")
        );
        let plain = [("url", "http://example.org/dav"), ("user", "me"), ("allow_plain_http", "true")];
        assert!(effective(&kind_of("webdav").unwrap(), &options(&plain)).is_ok());
        assert!(kind_of("local").is_err() && check_id("Photos").is_err() && check_id("photos-2_x").is_ok());
    }

    #[test]
    fn entries_round_trip_through_the_configuration() {
        let kind = kind_of("nextcloud").unwrap();
        let given = options(&[("server", "https://cloud.example.org"), ("user", "me"), ("cache_mb", "64")]);
        let table = table(&kind, &given);
        assert_eq!(table.get("cache_mb").and_then(toml::Value::as_integer), Some(64));
        let (known, read, options) = entry(&table).unwrap();
        assert_eq!((known.kind.as_str(), read, options["folder"].as_str()), ("nextcloud", given, ""));
        assert_eq!(hosts(&kind, &options), ["cloud.example.org"]);
        let mut written = table.clone();
        written.as_table_mut().unwrap().insert("password".into(), "hunter2".into());
        let error = entry(&written).unwrap_err();
        assert!(error.contains("is a secret") && !error.contains("hunter2"), "{error}");
    }
}
