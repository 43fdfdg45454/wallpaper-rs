use crate::db::{ensure, rows};
use crate::query::{Term, compile, terms};
use crate::{Error, Item, Kind, Library, PlaylistInfo, Result};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use wprs_schedule::Playlist;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub playlist: Playlist,
    pub uids: Vec<String>,
}

impl Library {
    pub fn resolve(
        &self,
        key: &str,
        now: i64,
        on_battery: bool,
        exclude: &HashSet<String>,
    ) -> Result<Resolved> {
        let PlaylistInfo { uid, rules, .. } = self.playlist(key)?;
        let skip_videos = on_battery && rules.skip_videos_on_battery;
        let video = |item: &Item| {
            item.kind == Kind::Video || item.package.as_ref().is_some_and(|p| p.kind == "video")
        };
        let playable = |item: &Item| {
            !item.missing
                && item.meta.error.is_none()
                && !exclude.contains(&item.uid)
                && !(skip_videos && video(item))
        };
        let items = self.playlist_items(&uid, now)?.into_iter().filter(playable);
        let (items, uids) = items.map(|item| (item.path, item.uid)).unzip();
        let (order, interval, per_output) = (rules.order, rules.interval, !rules.synced);
        Ok(Resolved { playlist: Playlist { id: uid, items, order, interval, per_output }, uids })
    }

    pub fn resolve_item(&self, reference: &str) -> Result<String> {
        let find = |sql: &str, value: &str| rows(&self.db, sql, [value], |r| r.get::<_, String>(0));
        let mut found = if Path::new(reference).is_absolute() {
            let path = fs::canonicalize(reference).ok().and_then(|path| path.to_str().map(String::from));
            find("SELECT uid FROM items WHERE path = ?1", path.as_deref().unwrap_or(reference))?
        } else {
            let upper = reference.to_ascii_uppercase();
            match find("SELECT uid FROM items WHERE uid = ?1", &upper)? {
                exact if !exact.is_empty() => exact,
                _ => {
                    let short =
                        || Error::Invalid("item", format!("{reference:?} is shorter than 6 characters"));
                    ensure(reference.chars().count() >= 6, short)?;
                    find("SELECT uid FROM items WHERE substr(uid, -length(?1)) = ?1", &upper)?
                }
            }
        };
        match found.len() {
            0 => Err(Error::NotFound("item", reference.into())),
            1 => Ok(found.remove(0)),
            n => Err(Error::Invalid("item", format!("{reference:?} matches {n} items; use more characters"))),
        }
    }

    pub fn normalize_query(&self, query: &str, seed: u32) -> Result<String> {
        compile(query, 0)?;
        let mut parts = Vec::new();
        for Term { negated, key, mut value, mut quoted } in terms(query) {
            match key.as_deref() {
                Some("in") => {
                    let playlist = match self.playlist(&value) {
                        Err(Error::NotFound(..)) => Err(Error::Query(format!("no playlist named {value:?}"))),
                        other => other,
                    }?;
                    ensure(playlist.query.is_none(), || Error::Query("in: needs a manual playlist".into()))?;
                    (value, quoted) = (playlist.uid, false);
                }
                Some("sort") if matches!(value.to_ascii_lowercase().as_str(), "random" | "random-") => {
                    value.insert_str(6, &format!(":{seed}"));
                }
                _ => {}
            }
            let (sign, key) = (if negated { "-" } else { "" }, key.map(|key| key + ":").unwrap_or_default());
            parts.push(if quoted {
                format!("{sign}{key}\"{value}\"")
            } else {
                format!("{sign}{key}{value}")
            });
        }
        Ok(parts.join(" "))
    }
}
