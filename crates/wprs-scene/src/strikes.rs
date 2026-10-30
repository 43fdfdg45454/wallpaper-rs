use crate::book::{Line, STRIKES, strike as add};
use crate::ledger::{blocked, book, edit, locked, now, unread};
use crate::trials::{hold, me};
use std::path::Path;

pub const STOPPED: &str = "it stopped the graphics card or the program twice; trying the left-out effects again gives it another chance";

pub fn keep_strikes(path: &Path) {
    let mut book = unread();
    if book.path.as_deref() != Some(path) {
        (book.held, book.mine) = (None, Vec::new());
    }
    (book.path, book.seen) = (Some(path.to_path_buf()), None);
}

pub fn struck(key: u64) -> bool {
    blocked(&book().lines, key)
}

pub fn trying(key: u64) -> Result<(), String> {
    let mut book = book();
    if book.mine.contains(&key) {
        return Ok(());
    }
    if blocked(&book.lines, key) {
        return Err(STOPPED.into());
    }
    if let Some(path) = book.path.clone().filter(|_| book.held.is_none()) {
        book.held = locked(&path, || hold(&path)).and_then(|held| held).ok();
    }
    if book.path.is_none() || book.held.is_some() {
        let mark = |lines: &mut Vec<Line>| {
            if !blocked(lines, key) {
                lines.push(Line::Trying(key, me().into()));
            }
        };
        drop(edit(&mut book, mark));
    }
    if blocked(&book.lines, key) {
        return Err(STOPPED.into());
    }
    book.mine.push(key);
    Ok(())
}

pub fn tried(key: u64) {
    let mut book = book();
    if let Some(at) = book.mine.iter().position(|k| *k == key) {
        book.mine.swap_remove(at);
        let mine = |l: &Line| matches!(l, Line::Trying(k, t) if *k == key && t == me());
        drop(edit(&mut book, |lines| lines.retain(|l| !mine(l))));
    }
}

pub fn end_trials() {
    let mut book = book();
    book.mine.clear();
    let mine = |l: &Line| matches!(l, Line::Trying(_, t) if t == me());
    if book.path.is_none() || book.held.is_some() {
        drop(edit(&mut book, |lines| lines.retain(|l| !mine(l))));
    }
}

pub fn strike(keys: &[u64]) -> std::io::Result<()> {
    let mut book = book();
    book.mine.retain(|k| !keys.contains(k));
    let at = now();
    edit(&mut book, |lines| keys.iter().for_each(|key| add(lines, *key, at)))
}

pub fn forgive_strikes() -> std::io::Result<usize> {
    let mut book = book();
    let before = book.lines.iter().filter(|l| matches!(l, Line::Struck(_, s, _) if *s >= STRIKES)).count();
    edit(&mut book, |lines| lines.retain(|l| matches!(l, Line::Trying(..))))?;
    Ok(before)
}
