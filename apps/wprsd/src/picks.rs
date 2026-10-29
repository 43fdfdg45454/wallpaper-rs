use crate::daemon::Daemon;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Duration, Instant, sleep_until};
use wprs_scene::{Control, Pick, Property, Values};

const IMAGES: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];
const SCAN: usize = 10_000;
const KEPT: usize = 16;

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn mix(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^ (x >> 33)
}

type Listing = (Vec<(PathBuf, Option<SystemTime>)>, Vec<PathBuf>);

static LISTED: LazyLock<Mutex<HashMap<(PathBuf, String), Listing>>> = LazyLock::new(Default::default);

fn stamp(dir: &Path) -> Option<SystemTime> {
    std::fs::metadata(dir).and_then(|m| m.modified()).ok()
}

fn walk(dir: &Path, max_files: usize, recursive: u8, fits: impl Fn(&Path) -> bool) -> Listing {
    let visible = |e: &std::fs::DirEntry| !e.file_name().to_string_lossy().starts_with('.');
    let (mut stamps, mut kept, mut dirs, mut seen) =
        (Vec::new(), BinaryHeap::new(), vec![(dir.to_owned(), 0)], 0);
    while let Some((dir, depth)) = dirs.pop() {
        stamps.push((dir.clone(), stamp(&dir)));
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten().filter(visible) {
            seen += 1;
            if seen > SCAN {
                return (stamps, kept.into_sorted_vec());
            }
            match entry.file_type() {
                Ok(t) if t.is_dir() && depth < recursive => dirs.push((entry.path(), depth + 1)),
                Ok(t) if t.is_file() && fits(&entry.path()) => kept.push(entry.path()),
                _ => {}
            }
            if kept.len() > max_files {
                kept.pop();
            }
        }
    }
    (stamps, kept.into_sorted_vec())
}

pub fn files(dir: &Path, prop: &Property) -> Vec<PathBuf> {
    let Control::Folder { max_files, recursive, .. } = prop.control else { return Vec::new() };
    let image = |p: &Path| {
        p.extension().is_some_and(|e| IMAGES.contains(&e.to_string_lossy().to_lowercase().as_str()))
    };
    let (images, videos) = (prop.accepts("image"), prop.accepts("video"));
    let fits = |p: &Path| (images && image(p)) || (videos && wprs_video::is_video(p));
    let key = (dir.to_owned(), format!("{max_files} {recursive} {images} {videos}"));
    let mut listed = LISTED.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let fresh = |(stamps, _): &&Listing| stamps.iter().all(|(d, at)| stamp(d) == *at);
    if let Some((_, files)) = listed.get(&key).filter(fresh) {
        return files.clone();
    }
    let found = walk(dir, max_files as usize, recursive, fits);
    if listed.len() >= KEPT {
        listed.clear();
    }
    listed.insert(key, found.clone());
    found.1
}

pub fn pick(dir: &Path, prop: &Property) -> Option<PathBuf> {
    let Control::Folder { pick, every, .. } = prop.control else { return None };
    let files = files(dir, prop);
    let turn = now() / u64::from(every.max(1));
    let at = match pick {
        Pick::Sequence => turn,
        Pick::Random => mix(turn ^ files.len() as u64),
    };
    files.get((at % files.len().max(1) as u64) as usize).cloned()
}

pub fn next_turn(props: &std::collections::BTreeMap<String, Property>, values: &Values) -> Option<Instant> {
    let chosen = |id: &String| values.get(id).and_then(|v| v.as_str()).is_some_and(|v| v.starts_with('/'));
    let every = props.iter().filter(|(id, _)| chosen(id)).filter_map(|(_, p)| match p.control {
        Control::Folder { every, .. } => Some(u64::from(every.max(1))),
        _ => None,
    });
    every.map(|e| Instant::now() + Duration::from_secs(e - now() % e)).min()
}

pub fn take(
    turns: &mut HashMap<String, Instant>,
    missed: &mut HashSet<String>,
    now: Instant,
    active: &HashSet<String>,
) -> Vec<String> {
    let due: Vec<String> = turns.iter().filter(|(_, at)| **at <= now).map(|(o, _)| o.clone()).collect();
    turns.retain(|o, _| !due.contains(o));
    let (shown, hidden): (Vec<String>, Vec<String>) = due.into_iter().partition(|o| active.contains(o));
    missed.extend(hidden);
    shown
}

pub fn back(
    turns: &mut HashMap<String, Instant>,
    missed: &mut HashSet<String>,
    shown: impl Fn(&str) -> bool,
) -> bool {
    let back: Vec<String> = missed.iter().filter(|o| shown(o)).cloned().collect();
    missed.retain(|o| !back.contains(o));
    turns.extend(back.iter().map(|o| (o.clone(), Instant::now())));
    !back.is_empty()
}

pub async fn run(daemon: Arc<Daemon>) {
    loop {
        let due = daemon.state().turns.values().min().copied();
        tokio::select! {
            () = sleep_until(due.unwrap_or_else(Instant::now)), if due.is_some() => {
                let outputs: Vec<String> = {
                    let mut guard = daemon.state();
                    let st = &mut *guard;
                    let active: HashSet<String> = st.outputs.iter().filter(|(_, i)| st.active(i)).map(|(o, _)| o.clone()).collect();
                    take(&mut st.turns, &mut st.missed, Instant::now(), &active)
                };
                let worker = daemon.clone();
                let _ = tokio::task::spawn_blocking(move || outputs.iter().for_each(|o| drop(worker.configure_output(o)))).await;
            }
            () = daemon.turned.notified() => {}
        }
    }
}

#[cfg(test)]
mod tests;
