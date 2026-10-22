use std::fs::{self, File, FileTimes};
use std::time::{Duration, UNIX_EPOCH};
use wiremock::matchers::{header, method};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};
use wprs_providers::{Cache, Item, LocalFolder, Media, Provider, client};

fn sample(id: &str, url: String) -> Item {
    Item { id: id.into(), title: "Sample".into(), url, attribution: None, license: None, media: Media::Image }
}

fn pixels() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_bytes(b"pixels".to_vec())
}

#[tokio::test]
async fn local_folder_scans_recursively_and_filters_by_extension() {
    let root = tempfile::tempdir().unwrap();
    for name in
        ["a.jpg", "sub/b.WEBM", "sub/deeper/c.png", "notes.txt", ".hidden/d.jpg", "sub/.e.png", "f.heic"]
    {
        let path = root.path().join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }
    let local = LocalFolder::new(root.path());
    let items = local.list().await.unwrap();
    let found: Vec<_> = items.iter().map(|item| (item.title.as_str(), item.media)).collect();
    assert_eq!(found, [("a.jpg", Media::Image), ("b.WEBM", Media::Video), ("c.png", Media::Image)]);

    let cache = Cache::new(root.path().join("cache"), 1).unwrap();
    assert_eq!(local.fetch(&items[1], &cache).await.unwrap(), root.path().join("sub/b.WEBM"));
}

#[tokio::test]
async fn cache_revalidates_with_etag_and_last_modified() {
    let server = MockServer::start().await;
    let modified = "Wed, 21 Oct 2026 07:28:00 GMT";
    let since =
        move |request: &Request| request.headers.get("if-modified-since").is_some_and(|v| v == modified);
    let fresh = Mock::given(header("If-None-Match", "\"v1\"")).and(since);
    fresh.respond_with(ResponseTemplate::new(304)).with_priority(1).expect(1).mount(&server).await;
    let full = pixels().insert_header("ETag", "\"v1\"").insert_header("Last-Modified", modified);
    Mock::given(method("GET")).respond_with(full).expect(1).mount(&server).await;

    let dir = tempfile::tempdir().unwrap();
    let (cache, client) = (Cache::new(dir.path(), 1 << 20).unwrap(), client().unwrap());
    let item = sample("sample", format!("{}/wall.png?size=large", server.uri()));
    let first = cache.download(&item, client.get(&item.url)).await.unwrap();
    assert_eq!((first.parent(), first.extension().unwrap().to_str()), (Some(dir.path()), Some("png")));
    let sidecar: serde_json::Value =
        serde_json::from_slice(&fs::read(format!("{}.json", first.display())).unwrap()).unwrap();
    assert_eq!((sidecar["etag"].as_str(), sidecar["modified"].as_str()), (Some("\"v1\""), Some(modified)));

    let second = cache.download(&item, client.get(&item.url)).await.unwrap();
    assert_eq!((second, fs::read(&first).unwrap()), (first, b"pixels".to_vec()));
}

#[tokio::test]
async fn cache_serves_the_stored_copy_when_the_server_is_gone() {
    let server = MockServer::builder().start().await;
    Mock::given(method("GET")).respond_with(pixels()).expect(1).mount(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (cache, client) = (Cache::new(dir.path(), 1 << 20).unwrap(), client().unwrap());
    let item = sample("offline", format!("{}/wall.jpg", server.uri()));
    let first = cache.download(&item, client.get(&item.url)).await.unwrap();
    drop(server);
    assert_eq!(cache.download(&item, client.get(&item.url)).await.unwrap(), first);

    let missing = sample("missing", item.url.replace("wall", "other"));
    assert!(cache.download(&missing, client.get(&missing.url)).await.is_err());
}

#[tokio::test]
async fn download_evicts_older_entries_but_keeps_the_newest() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(pixels()).expect(2).mount(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (cache, client) = (Cache::new(dir.path(), 4).unwrap(), client().unwrap());
    let (old, new) = (
        sample("old", format!("{}/old.jpg", server.uri())),
        sample("new", format!("{}/new.jpg", server.uri())),
    );
    let old_path = cache.download(&old, client.get(&old.url)).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    let new_path = cache.download(&new, client.get(&new.url)).await.unwrap();
    assert!(!old_path.exists() && new_path.exists());
}

#[test]
fn evict_removes_least_recently_accessed_media_first() {
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path(), 10).unwrap();
    for (name, accessed) in [("mid.png", 200), ("new.mp4", 300), ("old.jpg", 100)] {
        let path = dir.path().join(name);
        fs::write(&path, b"012345").unwrap();
        let times = FileTimes::new().set_accessed(UNIX_EPOCH + Duration::from_secs(accessed));
        File::options().write(true).open(&path).unwrap().set_times(times).unwrap();
    }
    fs::write(dir.path().join("old.jpg.json"), "{}").unwrap();
    fs::write(dir.path().join("notes.txt"), "not cached media, never evicted").unwrap();

    let removed = cache.evict().unwrap();
    assert_eq!(removed, [dir.path().join("old.jpg"), dir.path().join("mid.png")]);
    assert!(!dir.path().join("old.jpg.json").exists());
    assert!(dir.path().join("new.mp4").exists() && dir.path().join("notes.txt").exists());
}
