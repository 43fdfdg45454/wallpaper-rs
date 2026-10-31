use crate::error::{Result, invalid};
use reqwest::Url;
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::str::FromStr;
use wprs_core::model::{FieldKind, ProviderKind};
use wprs_providers::{Apod, Bing, Immich, Provider, Secret, Wallhaven, WebDav, WikimediaPotd, client};

mod fields;

pub use fields::effective;

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
mod tests;
