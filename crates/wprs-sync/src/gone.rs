use crate::{Ids, LIFETIME_MS, Replica, hex};
use std::collections::BTreeMap;

pub fn removed_package(repo_id: &str, id: &str) -> String {
    format!("pkg:{repo_id}/{id}")
}

pub fn removed_blob(hash: &[u8]) -> String {
    format!("blob:{}", hex(hash))
}

fn key(ids: &Ids, removed: &str) -> Option<String> {
    let item = match removed.split_once(':')? {
        ("pkg", rest) => rest.split_once('/').map(|(repo, id)| ids.library(repo, id))?,
        ("blob", text) => {
            let bytes = (0..text.len()).step_by(2).map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok());
            ids.blob(&bytes.collect::<Option<Vec<u8>>>()?)
        }
        _ => return None,
    };
    Some(format!("library/{item}"))
}

pub fn removals(ids: &Ids, removed: &BTreeMap<String, i64>, replica: &mut Replica, now_ms: u64) {
    let fresh = |at: &i64| (*at as u64).saturating_add(LIFETIME_MS) > now_ms;
    let keys = removed.iter().filter(|(_, at)| fresh(at));
    replica.gone.extend(keys.filter_map(|(removed, at)| Some((key(ids, removed)?, *at as u64))));
}

pub fn deleted_before(replica: &Replica, record: &str, added_at: i64) -> bool {
    let doc = replica.bases.get(record).map(|(_, doc)| doc).filter(|doc| doc.view().is_none());
    doc.is_some_and(|doc| !doc.is_cleared() && added_at.saturating_mul(1000) <= doc.clock().0 as i64)
}
