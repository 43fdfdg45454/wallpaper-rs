use crate::{Error, Kind, Result};
use rusqlite::types::Value;
use wprs_schedule::parse_interval;

const COLOR_DISTANCE: i64 = 3 * 40 * 40;
const TAGGED: &str =
    "i.id IN (SELECT it.item_id FROM item_tags it JOIN tags t ON t.id = it.tag_id WHERE t.name = ?)";
const LISTED: &str = "i.id IN (SELECT pi.item_id FROM playlist_items pi JOIN playlists p ON p.id = pi.playlist_id WHERE p.name = ?)";
const COLOR: &str = "(((i.color >> 16) & 255) - ?) * (((i.color >> 16) & 255) - ?) \
    + (((i.color >> 8) & 255) - ?) * (((i.color >> 8) & 255) - ?) + ((i.color & 255) - ?) * ((i.color & 255) - ?) <= ?";
const SIZES: [(&str, i64, i64); 6] = [
    ("8k", 7680, 4320),
    ("5k", 5120, 2880),
    ("4k", 3840, 2160),
    ("1440p", 2560, 1440),
    ("1080p", 1920, 1080),
    ("720p", 1280, 720),
];
const ORIENTATIONS: [(&str, &str); 3] = [
    ("portrait", "i.height > i.width"),
    ("landscape", "i.width > i.height"),
    ("square", "i.width = i.height"),
];
const KEYS: [(&str, &str); 15] = [
    ("tag", "a tag name"),
    ("kind", "image, video or package"),
    ("fav", "nothing, yes or no"),
    ("rating", "a rating from 0 to 5 with an optional >=, <=, >, < or ="),
    ("res", "a size like >=3840x2160, 4k, 1440p or 1080p"),
    ("orient", "portrait, landscape or square"),
    ("dur", "a duration like <30s, >2m or <=1h"),
    ("audio", "yes or no"),
    ("source", "a source id"),
    ("in", "a playlist name"),
    ("added", "an age like <7d"),
    ("used", "never or an age like <7d"),
    ("missing", "nothing, yes or no"),
    ("color", "a color like #3366ff"),
    ("sort", "added, title, rating, used or random, with a - suffix for descending"),
];

pub(crate) fn invalid(key: &str, value: &str) -> Error {
    let expected = KEYS.iter().find(|(k, _)| *k == key).map_or("", |(_, hint)| hint);
    Error::Query(format!("invalid value in \"{key}:{value}\", expected {expected}"))
}

pub(crate) fn condition(key: &str, value: &str, now: i64) -> Result<(String, Vec<Value>)> {
    if !KEYS.iter().any(|(k, _)| *k == key) {
        let known: Vec<&str> = KEYS.iter().map(|(k, _)| *k).collect();
        return Err(Error::Query(format!("unknown key \"{key}:\", expected one of {}", known.join(", "))));
    }
    parse(key, value, now).ok_or_else(|| invalid(key, value))
}

fn parse(key: &str, value: &str, now: i64) -> Option<(String, Vec<Value>)> {
    let lower = value.to_ascii_lowercase();
    let (op, rest) = comparison(&lower);
    let int = |n: i64| vec![Value::Integer(n)];
    let text = |text: &str| vec![Value::Text(text.into())];
    let (sql, params) = match key {
        "tag" | "in" | "source" | "audio" if value.is_empty() => return None,
        "tag" => (TAGGED.into(), text(value)),
        "in" => (LISTED.into(), text(value)),
        "source" => ("i.source_id = (SELECT id FROM sources WHERE uid = ?)".into(), text(value)),
        "kind" => ("i.kind = ?".into(), text(lower.parse::<Kind>().ok()?.as_str())),
        "fav" => ("i.favorite = ?".into(), int(flag(&lower)?)),
        "missing" => ("i.missing = ?".into(), int(flag(&lower)?)),
        "audio" => ("i.audio = ?".into(), int(flag(&lower)?)),
        "orient" => (ORIENTATIONS.iter().find(|(name, _)| *name == lower)?.1.into(), vec![]),
        "rating" => (
            format!("i.rating {} ?", op.unwrap_or("=")),
            int(rest.parse().ok().filter(|n| (0..=5).contains(n))?),
        ),
        "dur" => {
            let millis = parse_interval(rest)?.as_millis().try_into().ok()?;
            (format!("i.duration_ms {} ?", op.unwrap_or("<=")), int(millis))
        }
        "used" if lower == "never" => ("i.used_at IS NULL".into(), vec![]),
        "added" | "used" => {
            let column = if key == "added" { "i.added_at" } else { "i.used_at" };
            let age = parse_interval(rest)?.as_secs().try_into().ok()?;
            (format!("? - {column} {} ?", op.unwrap_or("<=")), vec![Value::Integer(now), Value::Integer(age)])
        }
        "res" => {
            let named = SIZES.iter().find(|(name, ..)| *name == rest).map(|(_, w, h)| (*w, *h));
            let parsed = || rest.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
            let ((w, h), op) = (named.or_else(parsed)?, op.unwrap_or(">="));
            (format!("i.width {op} ? AND i.height {op} ?"), vec![Value::Integer(w), Value::Integer(h)])
        }
        "color" => {
            let hex = lower.strip_prefix('#').unwrap_or(&lower);
            let hex = Some(hex).filter(|h| h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()))?;
            let rgb = i64::from_str_radix(hex, 16).ok()?;
            let [r, g, b] = [rgb >> 16, (rgb >> 8) & 255, rgb & 255];
            (COLOR.into(), [r, r, g, g, b, b, COLOR_DISTANCE].map(Value::Integer).to_vec())
        }
        _ => return None,
    };
    Some((sql, params))
}

fn comparison(value: &str) -> (Option<&'static str>, &str) {
    [">=", "<=", ">", "<", "="]
        .into_iter()
        .find_map(|op| value.strip_prefix(op).map(|rest| (Some(op), rest)))
        .unwrap_or((None, value))
}

fn flag(value: &str) -> Option<i64> {
    match value {
        "" | "yes" | "true" => Some(1),
        "no" | "false" => Some(0),
        _ => None,
    }
}
