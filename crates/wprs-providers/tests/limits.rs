use std::collections::HashSet;
use std::fs::{self, File, FileTimes};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::symlink;
use std::time::{Duration, UNIX_EPOCH};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wprs_providers::{Cache, Error, Item, Media, client};

fn sample(id: &str, url: String) -> Item {
    Item { id: id.into(), title: "Sample".into(), url, attribution: None, license: None, media: Media::Image }
}

fn aged(path: &std::path::Path, seconds: u64) {
    let at = UNIX_EPOCH + Duration::from_secs(seconds);
    File::options().write(true).open(path).unwrap().set_times(FileTimes::new().set_accessed(at)).unwrap();
}

#[tokio::test]
async fn the_room_left_is_enforced_while_streaming() {
    let server = MockServer::start().await;
    let big = ResponseTemplate::new(200).set_body_bytes(vec![1; 4096]);
    Mock::given(method("GET")).and(path("/big.png")).respond_with(big).mount(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let (cache, client) = (Cache::new(dir.path(), 1 << 20).unwrap(), client().unwrap());
    let item = sample("big", format!("{}/big.png", server.uri()));
    cache.allow(1000);
    assert!(matches!(cache.download(&item, client.get(&item.url)).await, Err(Error::TooLarge)));
    assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
    cache.allow(4096);
    assert_eq!(fs::read(cache.download(&item, client.get(&item.url)).await.unwrap()).unwrap().len(), 4096);
}

#[tokio::test]
async fn a_body_without_length_stops_at_the_room_left() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/stream.png", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let _ = stream.read(&mut [0; 1024]);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n");
            while stream.write_all(&[1; 4096]).is_ok() {}
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let (cache, client) = (Cache::new(dir.path(), 1 << 20).unwrap(), client().unwrap());
    cache.allow(64 << 10);
    let item = sample("stream", url);
    assert!(matches!(cache.download(&item, client.get(&item.url)).await, Err(Error::TooLarge)));
    assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
}

#[test]
fn eviction_resolves_links_and_stays_inside_the_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let (real, outside) = (tmp.path().join("real"), tmp.path().join("outside.jpg"));
    fs::create_dir(&real).unwrap();
    fs::write(&outside, vec![0; 100]).unwrap();
    for (n, name) in ["kept.jpg", "old.jpg", "new.jpg"].iter().enumerate() {
        fs::write(real.join(name), vec![0; 100]).unwrap();
        aged(&real.join(name), 1000 + n as u64);
    }
    symlink(&outside, real.join("linked.jpg")).unwrap();
    symlink(&real, tmp.path().join("cache")).unwrap();
    let cache = Cache::new(tmp.path().join("cache"), 100).unwrap();
    let removed = cache.evict_except(&HashSet::from([real.join("kept.jpg")])).unwrap();
    assert_eq!(removed, [real.join("old.jpg")]);
    assert!(real.join("kept.jpg").exists() && real.join("new.jpg").exists() && outside.exists());
}
