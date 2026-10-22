mod fixture;

use fixture::{Counting, DAWN, DAY, DUSK, NOW, Repo, file_url, files, signer};
use std::fs::{self, File};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;
use wprs_format::{
    Error, Fetch, FileFetch, add_repo, install, installed, preview, publish, repos, update_repo,
};

struct Signal(Sender<()>);

impl Fetch for Signal {
    fn get(&self, base: &str, path: &str, max: u64) -> io::Result<Option<Vec<u8>>> {
        let bytes = FileFetch.get(base, path, max);
        if path.starts_with("packages/") {
            self.0.send(()).unwrap();
        }
        bytes
    }
}

struct Halt(Counting, &'static str);

impl Fetch for Halt {
    fn get(&self, base: &str, path: &str, max: u64) -> io::Result<Option<Vec<u8>>> {
        if !path.starts_with(self.1) {
            return FileFetch.get(base, path, max);
        }
        self.0.get(base, path, max)?;
        Err(io::ErrorKind::Interrupted.into())
    }
}

fn hold(data: &Path, exclusive: bool) -> File {
    let mut options = File::options();
    let file = options.read(true).write(true).create(true).truncate(false).open(data.join(".repos.lock"));
    let file = file.unwrap();
    if exclusive {
        file.lock().unwrap()
    } else {
        file.lock_shared().unwrap()
    }
    file
}

#[test]
fn readers_wait_for_commits_and_repositories_are_added_once() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    let repo_id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    let (fetch, again) = (Counting::default(), format!("{}?repo_id={repo_id}", file_url(&repo.dir)));
    assert!(matches!(add_repo(&repo.data, &fetch, &again, NOW), Err(Error::Exists(_))));
    assert!(fetch.0.borrow().is_empty());
    let commit = hold(&repo.data, true);
    let data = repo.data.clone();
    let reader = thread::spawn(move || (repos(&data, NOW).unwrap().len(), installed(&data).unwrap().len()));
    thread::sleep(Duration::from_millis(200));
    assert!(!reader.is_finished());
    drop(commit);
    assert_eq!(reader.join().unwrap(), (1, 0));
}

#[test]
fn downloads_finish_before_the_lock_is_taken() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap();
    let (reading, (tx, rx)) = (hold(&repo.data, false), mpsc::channel());
    let data = repo.data.clone();
    let job = thread::spawn(move || install(&data, &Signal(tx), DAWN, NOW, false, false));
    rx.recv_timeout(Duration::from_secs(10)).unwrap();
    thread::sleep(Duration::from_millis(200));
    let dir = repo.data.join("packages").join(DAWN);
    let pending = files(&dir);
    assert!(pending.len() == 1 && pending[0].starts_with(".tmp-"), "{pending:?}");
    assert!(!job.is_finished() && installed(&repo.data).unwrap().is_empty());
    drop(reading);
    assert_eq!(job.join().unwrap().unwrap().version, "1.0.0");
    assert_eq!(files(&dir), ["1.0.0.wpak"]);
}

#[test]
fn scenes_are_refused_and_cancellation_skips_mirrors() {
    let repo = Repo::with(|tmp, s| s.mirrors = vec![file_url(&tmp.join("repo"))]);
    repo.add(DAWN, "1.0.0");
    repo.add(DUSK, "1.0.0");
    let catalog = repo.dir.join("catalog.json");
    let text = fs::read_to_string(&catalog).unwrap();
    let (head, tail) = text.split_at(text.find(DUSK).unwrap());
    fs::write(&catalog, head.to_string() + &tail.replacen("\"image\"", "\"scene\"", 1)).unwrap();
    repo.publish(NOW);
    let repo_id = add_repo(&repo.data, &FileFetch, &repo.url_at(&repo.copy("origin")), NOW).unwrap().repo_id;
    let fetch = Halt(Counting::default(), "packages/");
    let scene = install(&repo.data, &fetch, DUSK, NOW, false, false);
    assert!(matches!(&scene, Err(Error::Invalid { field, .. }) if field == "kind"), "{scene:?}");
    let halted = install(&repo.data, &fetch, DAWN, NOW, false, false);
    assert!(matches!(&halted, Err(Error::Io(e)) if e.kind() == io::ErrorKind::Interrupted), "{halted:?}");
    assert_eq!(fetch.0.count("packages/"), 1);
    assert!(files(&repo.data.join("packages").join(DAWN)).is_empty());
    let probe = update_repo(&repo.data, &Halt(Counting::default(), "entry/2"), &repo_id, NOW);
    assert!(matches!(&probe, Err(Error::Io(e)) if e.kind() == io::ErrorKind::Interrupted), "{probe:?}");
}

#[test]
fn listings_are_judged_at_the_latest_known_time() {
    let repo = Repo::with(|tmp, s| (s.mirrors, s.root_days) = (vec![file_url(&tmp.join("mirror"))], 30));
    publish(&repo.dir, &signer(&repo.online), 1, NOW).unwrap();
    let info = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap();
    assert_eq!(info.hosts, [file_url(&repo.dir), file_url(&repo.tmp.path().join("mirror"))]);
    assert_eq!((info.fingerprints, info.checked.timestamp()), (vec![repo.root.fingerprint.clone()], NOW));
    let early = repos(&repo.data, NOW - 365 * DAY).unwrap().remove(0);
    assert!(!early.stale && early.warnings.iter().any(|w| w.contains("index expires")), "{early:?}");
    let late = repos(&repo.data, NOW + 2 * DAY).unwrap().remove(0);
    assert!(late.stale && late.warnings.iter().any(|w| w.contains("stale")), "{late:?}");
    let expired = repos(&repo.data, NOW + 31 * DAY).unwrap().remove(0);
    assert!(expired.warnings.iter().any(|w| w.contains("root metadata has expired")), "{expired:?}");
}

#[test]
fn previews_are_capped_verified_and_private() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.add(DUSK, "1.0.0");
    let catalog = repo.dir.join("catalog.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&catalog).unwrap()).unwrap();
    value["packages"].as_array_mut().unwrap().iter_mut().filter(|p| p["id"] == DUSK).for_each(|p| {
        p["versions"][0]["preview"] = serde_json::Value::Null;
    });
    fs::write(&catalog, serde_json::to_vec(&value).unwrap()).unwrap();
    repo.publish(NOW);
    let repo_id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    let (cache, fetch) = (repo.tmp.path().join("cache"), |id, version, name| {
        preview(&repo.data, &FileFetch, &repo_id, id, version, &repo.tmp.path().join("cache").join(name))
    });
    assert!(fetch(DAWN, "1.0.0", "dawn").unwrap());
    assert_eq!(fs::read(cache.join("dawn")).unwrap(), b"RIFF\0\0\0\0WEBP");
    assert_eq!(fs::metadata(cache.join("dawn")).unwrap().permissions().mode() & 0o777, 0o600);
    assert!(!fetch(DUSK, "1.0.0", "dusk").unwrap());
    assert!(matches!(fetch(DAWN, "9.0.0", "none"), Err(Error::NotFound(_))));
    let published = repo.dir.join("previews").join(&files(&repo.dir.join("previews"))[0]);
    fs::write(&published, b"RIFF\0\0\0\0WEBP and more").unwrap();
    assert!(matches!(fetch(DAWN, "1.0.0", "long"), Err(Error::Mismatch(_))));
    fs::write(&published, b"RIFF\0\0\0\0WEBQ").unwrap();
    assert!(matches!(fetch(DAWN, "1.0.0", "flipped"), Err(Error::Mismatch(_))));
    assert_eq!(files(&cache), ["dawn"]);
}
