use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;
use std::sync::{Mutex, PoisonError};

const KEPT: usize = 4096;
const LONGEST: usize = 4096;
const MOST: u64 = 4 << 20;

type Refusal = Option<String>;

static TOUCHED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

fn parse(line: &str) -> Option<(&str, Refusal)> {
    let (hash, rest) = line.split_once(' ')?;
    match rest.split_once(' ').unwrap_or((rest, "")) {
        ("ok", "") => Some((hash, None)),
        ("refused", why) => Some((hash, Some(serde_json::from_str(why).ok()?))),
        _ => None,
    }
}

fn line(hash: &str, refusal: &Refusal) -> String {
    match refusal {
        None => format!("{hash} ok\n"),
        Some(why) => format!("{hash} refused {}\n", serde_json::Value::from(why.as_str())),
    }
}

fn text(path: &Path) -> String {
    let mut text = String::new();
    let file = std::fs::File::open(path);
    drop(file.map(|f| f.take(MOST).read_to_string(&mut text)));
    text
}

pub(crate) fn read(path: &Path, hash: &str) -> Option<Refusal> {
    let found = text(path).lines().filter_map(parse).find(|(h, _)| *h == hash).map(|(_, refusal)| refusal);
    if found.is_some() {
        TOUCHED.lock().unwrap_or_else(PoisonError::into_inner).insert(hash.to_owned());
    }
    found
}

pub(crate) fn write(path: &Path, hash: &str, refusal: &Refusal) -> std::io::Result<()> {
    if refusal.as_ref().is_some_and(|why| why.len() > LONGEST) {
        return Ok(());
    }
    crate::ledger::locked(path, || {
        let found = text(path);
        let touched = std::mem::take(&mut *TOUCHED.lock().unwrap_or_else(PoisonError::into_inner));
        let kept = found.lines().filter_map(|l| parse(l).filter(|(h, _)| *h != hash).map(|(h, _)| (h, l)));
        let (recent, old): (Vec<_>, Vec<_>) = kept.partition(|(h, _)| touched.contains(*h));
        let mut lines: Vec<String> = old.into_iter().chain(recent).map(|(_, l)| format!("{l}\n")).collect();
        lines.push(line(hash, refusal));
        let temp = path.with_extension("new");
        std::fs::write(&temp, lines[lines.len().saturating_sub(KEPT)..].concat())?;
        std::fs::rename(&temp, path).inspect_err(|_| drop(std::fs::remove_file(&temp)))
    })?
}

#[cfg(test)]
mod tests {
    #[test]
    fn verdicts_read_again_outlive_older_ones() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("checked-effects");
        let full: String = (0..super::KEPT).map(|n| format!("{n:064x} ok\n")).collect();
        std::fs::write(&path, full).unwrap();
        let first = format!("{:064x}", 0);
        assert_eq!(super::read(&path, &first), Some(None));
        super::write(&path, &format!("{:064x}", 1 << 20), &None).unwrap();
        assert_eq!(super::read(&path, &first), Some(None), "a verdict read again is kept");
        assert_eq!(super::read(&path, &format!("{:064x}", 1)), None, "the oldest unread one is dropped");
    }
}
