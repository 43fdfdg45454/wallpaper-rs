use crate::daemon::Daemon;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Duration, Instant, sleep_until};
use wprs_scene::{Control, Pick, Property, Values};

const IMAGES: [&str; 6] = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];
const MAX_FILES: usize = 1000;

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn mix(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^ (x >> 33)
}

pub fn pick(dir: &Path, prop: &Property, pick: Pick, every: u32) -> Option<PathBuf> {
    let image = |p: &Path| {
        p.extension().is_some_and(|e| IMAGES.contains(&e.to_string_lossy().to_lowercase().as_str()))
    };
    let fits =
        |p: &Path| (prop.accepts("image") && image(p)) || (prop.accepts("video") && wprs_video::is_video(p));
    let visible = |e: &std::fs::DirEntry| !e.file_name().to_string_lossy().starts_with('.');
    let entries = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| visible(e) && e.file_type().is_ok_and(|t| t.is_file()));
    let mut files: Vec<PathBuf> = entries.map(|e| e.path()).filter(|p| fits(p)).take(MAX_FILES).collect();
    files.sort();
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

pub async fn run(daemon: Arc<Daemon>) {
    loop {
        let due = daemon.state().turns.values().min().copied();
        tokio::select! {
            () = sleep_until(due.unwrap_or_else(Instant::now)), if due.is_some() => {
                let outputs: Vec<String> = {
                    let mut st = daemon.state();
                    let now = Instant::now();
                    let due: Vec<String> = st.turns.iter().filter(|(_, at)| **at <= now).map(|(o, _)| o.clone()).collect();
                    st.turns.retain(|o, _| !due.contains(o));
                    let shown = |o: &String| st.outputs.get(o).is_some_and(|i| st.active(i));
                    let (shown, hidden): (Vec<String>, Vec<String>) = due.into_iter().partition(shown);
                    st.missed.extend(hidden);
                    shown
                };
                let worker = daemon.clone();
                let _ = tokio::task::spawn_blocking(move || outputs.iter().for_each(|o| drop(worker.configure_output(o)))).await;
            }
            () = daemon.turned.notified() => {}
        }
    }
}
