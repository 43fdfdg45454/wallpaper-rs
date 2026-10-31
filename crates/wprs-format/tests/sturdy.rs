mod fixture;

use fixture::{DAWN, DUSK, NOW, Repo, copy_dir, file_url, key, signer};
use std::cell::{Cell, RefCell};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use wprs_format::{
    Error, Fetch, FileFetch, Reason, add_repo, browse, comment, current, install, installed, previews,
    publish, release, remove_repo, repos, revoke, rotate_root, settings, sign_root, update_repo,
};

struct Hook<F: Fn()>(Cell<bool>, F);

impl<F: Fn()> Fetch for Hook<F> {
    fn get(&self, base: &str, path: &str, max: u64) -> io::Result<Option<Vec<u8>>> {
        if path.starts_with("packages/") && !self.0.replace(true) {
            (self.1)();
        }
        FileFetch.get(base, path, max)
    }
}

struct Net(PathBuf, RefCell<Vec<String>>);

impl Fetch for Net {
    fn get(&self, base: &str, path: &str, max: u64) -> io::Result<Option<Vec<u8>>> {
        self.1.borrow_mut().push(base.into());
        let local = file_url(&self.0);
        FileFetch.get(if base == "https://repo.test/" { &local } else { base }, path, max)
    }

    fn copy(&self, base: &str, path: &str, max: u64, out: &mut dyn Write) -> io::Result<bool> {
        let Some(bytes) = self.get(base, path, max)? else { return Ok(false) };
        out.write_all(&bytes).map(|()| true)
    }
}

fn dir(repo: &Repo, id: &str) -> PathBuf {
    repo.data.join("repos").join(id)
}

#[test]
fn broken_folders_are_reported_and_recovered() {
    let (repo, other) = (Repo::new(), Repo::new());
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    other.add(DUSK, "1.0.0");
    other.publish(NOW);
    let id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    let id2 = add_repo(&repo.data, &FileFetch, &other.url(), NOW).unwrap().repo_id;
    fs::write(dir(&repo, &id).join("root.json"), b"{").unwrap();
    let listed = repos(&repo.data, NOW).unwrap();
    let broken = listed.iter().find(|r| r.repo_id == id).unwrap();
    assert!(broken.broken.is_some() && broken.url == repo.url(), "{broken:?}");
    assert!(listed.iter().any(|r| r.repo_id == id2 && r.broken.is_none()));
    assert_eq!(browse(&repo.data, "").unwrap().iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), [DUSK]);
    install(&repo.data, &FileFetch, DUSK, NOW, false, false).unwrap();
    assert!(matches!(install(&repo.data, &FileFetch, DAWN, NOW, false, false), Err(Error::NotFound(_))));
    assert_eq!(update_repo(&repo.data, &FileFetch, &id, NOW + 1).unwrap().repo.packages, 1);
    fs::remove_file(dir(&repo, &id).join("entry.json")).unwrap();
    assert_eq!(add_repo(&repo.data, &FileFetch, &repo.url(), NOW + 1).unwrap().packages, 1);
    fs::remove_file(dir(&repo, &id).join("state.json")).unwrap();
    assert!(repos(&repo.data, NOW).unwrap().iter().any(|r| r.repo_id == id && r.broken.is_some()));
    assert!(matches!(update_repo(&repo.data, &FileFetch, &id, NOW + 2), Err(Error::Missing(_))));
    assert_eq!(add_repo(&repo.data, &FileFetch, &repo.url(), NOW + 2).unwrap().repo_id, id);
    assert!(repos(&repo.data, NOW).unwrap().iter().all(|r| r.broken.is_none()));
    fs::remove_file(dir(&repo, &id).join("state.json")).unwrap();
    remove_repo(&repo.data, &id).unwrap();
    assert_eq!(repos(&repo.data, NOW).unwrap().len(), 1);
}

#[test]
fn concurrent_updates_never_roll_back() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    let id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    repo.publish(NOW + 10);
    let old = repo.copy("old");
    repo.publish(NOW + 20);
    let outer = Hook(Cell::new(false), || drop(update_repo(&repo.data, &FileFetch, &id, NOW + 30).unwrap()));
    struct Entry<'a, F: Fn()>(&'a Hook<F>, &'a Path);
    impl<F: Fn()> Fetch for Entry<'_, F> {
        fn get(&self, _: &str, path: &str, max: u64) -> io::Result<Option<Vec<u8>>> {
            if path == "entry/latest" && !self.0.0.replace(true) {
                (self.0.1)();
            }
            FileFetch.get(&file_url(self.1), path, max)
        }
    }
    let stale = update_repo(&repo.data, &Entry(&outer, &old), &id, NOW + 30).unwrap();
    assert_eq!((stale.repo.entry_version, repos(&repo.data, NOW).unwrap()[0].entry_version), (3, 3));
}

#[test]
fn installs_recheck_the_repository_before_they_finish() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.add(DUSK, "1.0.0");
    repo.publish(NOW);
    let id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    let revoking = Hook(Cell::new(false), || {
        revoke(&repo.dir, DAWN, Some("1.0.0"), Reason::Broken, "").unwrap();
        repo.publish(NOW + 10);
        update_repo(&repo.data, &FileFetch, &id, NOW + 20).unwrap();
    });
    assert!(matches!(install(&repo.data, &revoking, DAWN, NOW + 20, false, false), Err(Error::Revoked(_))));
    let done = install(&repo.data, &FileFetch, DUSK, NOW + 20, false, false).unwrap();
    assert_eq!(current(&repo.data, &id, DUSK, "1.0.0").unwrap(), done);
    remove_repo(&repo.data, &id).unwrap();
    assert!(matches!(current(&repo.data, &id, DUSK, "1.0.0"), Err(Error::NotFound(_))));
    let again = add_repo(&repo.data, &FileFetch, &repo.url(), NOW + 20).unwrap().repo_id;
    let removing = Hook(Cell::new(false), || remove_repo(&repo.data, &again).unwrap());
    fs::remove_dir_all(repo.data.join("packages")).unwrap();
    fs::remove_file(repo.data.join("installed.json")).unwrap();
    assert!(matches!(install(&repo.data, &removing, DUSK, NOW + 20, false, false), Err(Error::NotFound(_))));
    assert!(installed(&repo.data).unwrap().is_empty());
}

#[test]
fn file_mirrors_serve_only_local_repositories() {
    let repo = Repo::with(|tmp, s| s.mirrors = vec![file_url(&tmp.join("repo"))]);
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    let net = repo.copy("net");
    fs::remove_dir_all(net.join("packages")).unwrap();
    let fetch = Net(net, RefCell::default());
    let url = format!("https://repo.test/?fingerprint={}", repo.root.fingerprint);
    let info = add_repo(&repo.data, &fetch, &url, NOW).unwrap();
    assert_eq!(info.hosts, ["https://repo.test/"]);
    assert!(matches!(install(&repo.data, &fetch, DAWN, NOW, false, false), Err(Error::Missing(_))));
    assert!(fetch.1.borrow().iter().all(|base| base == "https://repo.test/"));
}

#[test]
fn every_version_names_its_preview() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    let id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    let item = &browse(&repo.data, "").unwrap()[0];
    assert_eq!(item.repo_name, "Example wallpapers");
    let first = release(&repo.data, &id, DAWN, "1.0.0").unwrap().preview.unwrap().sha256;
    assert_eq!(previews(&repo.data, &id).unwrap(), [first]);
    assert!(matches!(release(&repo.data, &id, DAWN, "9.0.0"), Err(Error::NotFound(_))));
}

#[test]
fn recovery_never_goes_back_to_an_older_root() {
    let repo = Repo::new();
    repo.add(DAWN, "1.0.0");
    repo.publish(NOW);
    let id = add_repo(&repo.data, &FileFetch, &repo.url(), NOW).unwrap().repo_id;
    let old = repo.copy("old");
    let (root2, online2) = (key(), key());
    let mut next = settings(&repo.dir).unwrap();
    (next.root_keys, next.online_keys) = (vec![root2.public.clone()], vec![online2.public.clone()]);
    rotate_root(&repo.dir, &next, NOW + 10).unwrap();
    let json = fs::read(repo.dir.join("root/2.json")).unwrap();
    let by_old = signer(&repo.root).sign(&json, &comment("root", 2, &id)).unwrap();
    fs::write(repo.dir.join("root/2.json.minisig"), by_old).unwrap();
    sign_root(&repo.dir, &signer(&root2)).unwrap();
    publish(&repo.dir, &signer(&online2), 7, NOW + 20).unwrap();
    assert_eq!(update_repo(&repo.data, &FileFetch, &id, NOW + 30).unwrap().repo.root_version, 2);
    fs::write(dir(&repo, &id).join("root.json"), b"{").unwrap();
    fs::remove_dir_all(&repo.dir).unwrap();
    copy_dir(&old, &repo.dir);
    let refused = update_repo(&repo.data, &FileFetch, &id, NOW + 40);
    assert!(matches!(&refused, Err(Error::Rollback(_))), "{:?}", refused.map(|o| o.repo.root_version));
    assert!(repos(&repo.data, NOW).unwrap().iter().any(|r| r.repo_id == id && r.root_version == 2));
}
