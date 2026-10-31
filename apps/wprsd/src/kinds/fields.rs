use super::{Options, SECRETS, server};
use crate::error::{Result, invalid};
use serde_json::Value;
use wprs_core::model::{Field, FieldKind, ProviderKind};

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

pub(super) fn text(value: &Value, kind: FieldKind) -> String {
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
