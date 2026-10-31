use crate::db::{ensure, rows};
use crate::filter::{condition, invalid};
use crate::items::{SELECT_ITEMS, item_row};
use crate::{Error, Item, Library, Result};
use rusqlite::params_from_iter;
use rusqlite::types::Value;

const MATCHES: &str = "i.id IN (SELECT rowid FROM items_fts WHERE items_fts MATCH ?)";

pub(crate) struct Compiled {
    filter: String,
    params: Vec<Value>,
    order: String,
}

pub(crate) struct Term {
    pub(crate) negated: bool,
    pub(crate) key: Option<String>,
    pub(crate) value: String,
    pub(crate) quoted: bool,
}

impl Library {
    pub fn search(&self, query: &str, now: i64, offset: u32, limit: u32) -> Result<Vec<Item>> {
        let Compiled { filter, mut params, order } = compile(query, now)?;
        params.extend([Value::Integer(limit.into()), Value::Integer(offset.into())]);
        let sql = format!("{SELECT_ITEMS} WHERE {filter} ORDER BY {order} LIMIT ? OFFSET ?");
        rows(&self.db, &sql, params_from_iter(params), item_row)
    }

    pub fn count(&self, query: &str, now: i64) -> Result<u32> {
        let Compiled { filter, params, .. } = compile(query, now)?;
        let sql = format!("SELECT count(*) FROM items i WHERE {filter}");
        Ok(self.db.query_row(&sql, params_from_iter(params), |row| row.get(0))?)
    }
}

pub fn shown(query: &str, hide_mature: bool) -> String {
    match hide_mature && !query.contains("content:") {
        true => format!("{query} -content:mature"),
        false => query.to_owned(),
    }
}

pub(crate) fn compile(query: &str, now: i64) -> Result<Compiled> {
    ensure(query.matches('"').count().is_multiple_of(2), || Error::Query("a quote is not closed".into()))?;
    let order = "i.added_at DESC, i.id DESC".to_string();
    let mut compiled = Compiled { filter: "1".into(), params: Vec::new(), order };
    for Term { negated, key, value, quoted } in terms(query) {
        let (sql, params) = match key.as_deref() {
            Some("sort") => {
                ensure(!negated, || Error::Query("sort: cannot be negated".into()))?;
                compiled.order = sort(&value.to_ascii_lowercase()).ok_or_else(|| invalid("sort", &value))?;
                continue;
            }
            Some(key) => condition(key, &value, now)?,
            None if value.chars().any(char::is_alphanumeric) => {
                let phrase = format!("\"{}\"{}", value.replace('"', "\"\""), if quoted { "" } else { "*" });
                (MATCHES.to_string(), vec![Value::Text(phrase)])
            }
            None => continue,
        };
        let not = if negated { "NOT " } else { "" };
        compiled.filter += &format!(" AND {not}coalesce(({sql}), 0)");
        compiled.params.extend(params);
    }
    Ok(compiled)
}

pub(crate) fn terms(query: &str) -> Vec<Term> {
    let mut chars = query.chars().peekable();
    let mut terms = Vec::new();
    while chars.peek().is_some() {
        if chars.next_if(|c| c.is_whitespace()).is_some() {
            continue;
        }
        let negated = chars.next_if_eq(&'-').is_some();
        let mut term = Term { negated, key: None, value: String::new(), quoted: false };
        while let Some(c) = chars.next_if(|c| !c.is_whitespace()) {
            let starts_key = term.key.is_none() && !term.quoted && !term.value.is_empty();
            match c {
                '"' => {
                    term.quoted = true;
                    term.value.extend(chars.by_ref().take_while(|c| *c != '"'));
                }
                ':' if starts_key && term.value.chars().all(|c| c.is_ascii_alphabetic()) => {
                    term.key = Some(std::mem::take(&mut term.value).to_ascii_lowercase());
                }
                c => term.value.push(c),
            }
        }
        terms.push(term);
    }
    terms
}

fn sort(value: &str) -> Option<String> {
    let (name, descending) = value.strip_suffix('-').map_or((value, false), |name| (name, true));
    let column = match name {
        "added" => "i.added_at".into(),
        "title" => "i.title COLLATE NOCASE".into(),
        "rating" => "i.rating".into(),
        "used" => "i.used_at".into(),
        "random" => "random()".into(),
        _ => format!("(i.id * {} + 7) % 2147483647", name.strip_prefix("random:")?.parse::<u32>().ok()?),
    };
    Some(format!("{column} {}, i.id", if descending { "DESC" } else { "ASC" }))
}
