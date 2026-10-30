use crate::trials::me;
use std::path::Path;

pub const STRIKES: u32 = 2;
pub const FORGIVEN: u64 = 86_400;
const KEPT: usize = 1024;
const BLOCKED: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Line {
    Struck(u64, u32, u64),
    Trying(u64, String),
}

pub(crate) fn parse(text: &str, now: u64, alive: &dyn Fn(&str) -> bool) -> Vec<Line> {
    let line = |line: &str| {
        let mut words = line.split_whitespace();
        let key = u64::from_str_radix(words.next()?, 16).ok()?;
        match words.next()? {
            "trying" => Some(Line::Trying(key, words.next()?.to_string())),
            n => Some(Line::Struck(key, n.parse::<u32>().ok()?.min(STRIKES), words.next()?.parse().ok()?)),
        }
    };
    let (mut lines, mut dead) = (Vec::new(), Vec::new());
    for found in text.lines().filter_map(line) {
        match found {
            Line::Trying(key, token) if !alive(&token) => dead.push(key),
            found => lines.push(found),
        }
    }
    dead.into_iter().for_each(|key| strike(&mut lines, key, now));
    let kept = |l: &Line| match l {
        Line::Struck(_, strikes, at) => *strikes >= STRIKES || now.saturating_sub(*at) < FORGIVEN,
        Line::Trying(..) => true,
    };
    lines.retain(kept);
    trim(&mut lines, false, KEPT);
    trim(&mut lines, true, BLOCKED);
    lines
}

fn trim(lines: &mut Vec<Line>, blocked: bool, cap: usize) {
    let mut ages: Vec<(u64, usize)> = (lines.iter().enumerate())
        .filter_map(|(i, l)| match l {
            Line::Struck(_, strikes, at) if (*strikes >= STRIKES) == blocked => Some((*at, i)),
            _ => None,
        })
        .collect();
    let Some(over) = ages.len().checked_sub(cap).filter(|over| *over > 0) else { return };
    ages.sort_unstable();
    let mut gone = vec![false; lines.len()];
    ages[..over].iter().for_each(|(_, i)| gone[*i] = true);
    let mut at = gone.into_iter();
    lines.retain(|_| !at.next().unwrap_or(false));
}

pub(crate) fn strike(lines: &mut Vec<Line>, key: u64, now: u64) {
    let mine = |l: &Line| matches!(l, Line::Trying(k, token) if *k == key && token == me());
    lines.retain(|l| !mine(l));
    let at = lines.iter().position(|l| matches!(l, Line::Struck(k, ..) if *k == key));
    let strikes = at.map_or(0, |i| match lines.remove(i) {
        Line::Struck(_, s, _) => s,
        Line::Trying(..) => 0,
    });
    lines.push(Line::Struck(key, strikes.saturating_add(1).min(STRIKES), now));
}

pub(crate) fn text(lines: &[Line]) -> String {
    let line = |l: &Line| match l {
        Line::Struck(key, strikes, at) => format!("{key:016x} {strikes} {at}\n"),
        Line::Trying(key, token) => format!("{key:016x} trying {token}\n"),
    };
    lines.iter().map(line).collect()
}

pub(crate) fn save(path: &Path, lines: &[Line]) -> std::io::Result<()> {
    let temp = path.with_extension("new");
    std::fs::write(&temp, text(lines))?;
    std::fs::rename(&temp, path).inspect_err(|_| drop(std::fs::remove_file(&temp)))
}
