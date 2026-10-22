use crate::compositor::lock;
use std::fs::{self, File};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, SystemTime};
use tokio::sync::oneshot;
use wprs_library::Item;

pub const SIZES: [u32; 3] = [256, 512, 1024];

pub enum Req {
    Wait(u32, oneshot::Sender<Result<PathBuf, String>>),
    Signal(String, u32),
    Analyze(std::sync::mpsc::Sender<bool>),
}

impl Req {
    fn rank(&self) -> u8 {
        match self {
            Req::Wait(..) => 0,
            Req::Signal(..) => 1,
            Req::Analyze(_) => 2,
        }
    }

    pub fn size(&self) -> u32 {
        match self {
            Req::Wait(size, _) | Req::Signal(_, size) => *size,
            Req::Analyze(_) => SIZES[0],
        }
    }
}

pub struct Task {
    pub item: Item,
    pub reqs: Vec<Req>,
}

pub struct Thumbs {
    pub root: PathBuf,
    queue: Mutex<Vec<Task>>,
    ready: Condvar,
    used: AtomicU64,
}

pub fn key(path: &Path, mtime: i64, size: u64) -> String {
    blake3::hash(format!("{}\0{mtime}\0{size}", path.to_string_lossy()).as_bytes()).to_hex().to_string()
}

impl Thumbs {
    pub fn new(cache: &Path) -> Self {
        let (queue, ready, used) = (Mutex::default(), Condvar::new(), AtomicU64::new(0));
        Self { root: cache.join("thumbs"), queue, ready, used }
    }

    pub fn path(&self, item: &Item, size: u32) -> PathBuf {
        let key = key(&item.path, item.mtime, item.size);
        self.root.join(size.to_string()).join(format!("{key}.jpg"))
    }

    pub fn cached(&self, item: &Item, size: u32) -> Option<PathBuf> {
        let path = self.path(item, size);
        File::open(&path).and_then(|f| f.set_modified(SystemTime::now())).ok().map(|()| path)
    }

    pub fn remove(&self, item: &Item) {
        SIZES.iter().for_each(|size| drop(fs::remove_file(self.path(item, *size))));
    }

    pub fn queue(&self, item: Item, req: Req) {
        let mut queue = lock(&self.queue);
        let same = |r: &Req| matches!((r, &req), (Req::Signal(a, x), Req::Signal(b, y)) if a == b && x == y);
        match queue.iter_mut().find(|task| task.item.uid == item.uid) {
            Some(task) if task.reqs.iter().any(same) => {}
            Some(task) => task.reqs.push(req),
            None => queue.push(Task { item, reqs: vec![req] }),
        }
        self.ready.notify_one();
    }

    pub fn forget(&self, sender: &str) {
        let mut queue = lock(&self.queue);
        let other = |req: &Req| !matches!(req, Req::Signal(s, _) if s == sender);
        queue.iter_mut().for_each(|task| task.reqs.retain(other));
        queue.retain(|task| !task.reqs.is_empty());
    }

    pub fn forget_analysis(&self) {
        let mut queue = lock(&self.queue);
        queue.iter_mut().for_each(|task| task.reqs.retain(|req| !matches!(req, Req::Analyze(_))));
        queue.retain(|task| !task.reqs.is_empty());
    }

    pub fn next(&self, idle: Option<Duration>) -> Option<Task> {
        let rank = |task: &Task| task.reqs.iter().map(Req::rank).min().unwrap_or(u8::MAX);
        let mut queue = lock(&self.queue);
        loop {
            if let Some(index) = (0..queue.len()).min_by_key(|i| (rank(&queue[*i]), *i)) {
                return Some(queue.remove(index));
            }
            queue = match idle {
                Some(idle) => match self.ready.wait_timeout(queue, idle) {
                    Ok((_, timeout)) if timeout.timed_out() => return None,
                    Ok((queue, _)) => queue,
                    Err(e) => e.into_inner().0,
                },
                None => self.ready.wait(queue).unwrap_or_else(|e| e.into_inner()),
            };
        }
    }

    pub fn added(&self, bytes: u64, limit_mb: u32) {
        let limit = u64::from(limit_mb) << 20;
        if self.used.fetch_add(bytes, Ordering::Relaxed) + bytes > limit {
            self.prune(limit);
        }
    }

    pub fn prune(&self, limit: u64) {
        let previews = self.root.with_file_name("previews");
        let dirs = SIZES.map(|size| self.root.join(size.to_string()));
        let dirs = dirs.iter().chain([&previews]);
        let entries = dirs.filter_map(|dir| fs::read_dir(dir).ok()).flatten().flatten();
        let mut files: Vec<(i64, u64, PathBuf)> = (entries
            .filter_map(|e| Some((e.metadata().ok()?, e.path()))))
        .filter(|(meta, _)| meta.is_file())
        .map(|(meta, path)| (meta.mtime(), meta.len(), path))
        .collect();
        let mut total: u64 = files.iter().map(|(_, len, _)| len).sum();
        if total > limit {
            files.sort();
            for (_, len, path) in files {
                if total <= limit / 10 * 9 {
                    break;
                }
                if fs::remove_file(path).is_ok() {
                    total -= len;
                }
            }
        }
        self.used.store(total, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_follow_path_mtime_and_size() {
        let key = key(Path::new("/pictures/a.png"), 1_700_000_000, 42);
        let expected = blake3::hash(b"/pictures/a.png\x001700000000\x0042").to_hex().to_string();
        assert_eq!(key, expected);
        assert_eq!(key.len(), 64);
        assert_ne!(super::key(Path::new("/pictures/a.png"), 1_700_000_001, 42), key);
    }

    #[test]
    fn pruning_deletes_the_oldest_files_first() {
        let dir = tempfile::tempdir().unwrap();
        let thumbs = Thumbs::new(dir.path());
        let small = thumbs.root.join("256");
        fs::create_dir_all(&small).unwrap();
        for (n, age) in [(1, 30), (2, 20), (3, 10)] {
            let path = small.join(format!("{n}.jpg"));
            fs::write(&path, [0; 400]).unwrap();
            let at = SystemTime::now() - Duration::from_secs(age);
            File::options().write(true).open(&path).unwrap().set_modified(at).unwrap();
        }
        thumbs.added(1, 0);
        assert!(!small.join("3.jpg").exists());
        thumbs.prune(1000);
        assert_eq!(thumbs.used.load(Ordering::Relaxed), 0);
        fs::write(small.join("4.jpg"), [0; 400]).unwrap();
        fs::write(small.join("5.jpg"), [0; 400]).unwrap();
        thumbs.prune(1000);
        assert_eq!(thumbs.used.load(Ordering::Relaxed), 800);
        fs::write(small.join("6.jpg"), [0; 400]).unwrap();
        thumbs.prune(1000);
        assert!(thumbs.used.load(Ordering::Relaxed) <= 900);
    }
}
