use super::*;
use crate::kinds::{Options, effective, kind_of};
use serde_json::json;
use std::collections::HashSet;
use wiremock::matchers::{header, header_exists, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wprs_core::dbus::error::NEEDS_SECRET;
use wprs_providers::{Bing, Provider};

async fn pictures() -> MockServer {
    let server = MockServer::start().await;
    let image = |n: u32| {
        json!({"url": format!("/p{n}.jpg"), "urlbase": format!("/th?id=Pic{n}"),
            "title": format!("Picture {n}"), "copyright": "Example Studio"})
    };
    let archive = json!({"images": (1..=3).map(image).collect::<Vec<_>>()});
    let listed = ResponseTemplate::new(200).set_body_json(archive);
    Mock::given(path("/HPImageArchive.aspx")).respond_with(listed).mount(&server).await;
    let same = Mock::given(path("/th")).and(header_exists("if-none-match"));
    same.respond_with(ResponseTemplate::new(304)).with_priority(1).mount(&server).await;
    let picture = ResponseTemplate::new(200).insert_header("etag", "\"v1\"").set_body_bytes(vec![7; 1000]);
    Mock::given(path("/th")).respond_with(picture).mount(&server).await;
    server
}

#[tokio::test]
async fn downloads_respect_the_budget_and_reuse_cached_files() {
    let (server, dir) = (pictures().await, tempfile::tempdir().unwrap());
    let provider = Bing { base: server.uri(), ..Bing::new(wprs_providers::client().unwrap(), "en-US", true) };
    let cache = Cache::new(dir.path().join("daily"), 1 << 20).unwrap();
    let none = HashSet::new();
    let first = download(&provider, &cache, &none, 2500, |_, _| true).await.unwrap().unwrap();
    assert_eq!((first.listed, first.downloaded, first.reused, first.skipped, first.failed), (3, 2, 0, 1, 0));
    assert_eq!(first.files[1].1.title, "Picture 2");
    let third = cache.path(&provider.list().await.unwrap()[2]);
    assert!(!third.exists());
    let protected = HashSet::from([third.clone()]);
    let second = download(&provider, &cache, &protected, 2500, |_, _| true).await.unwrap().unwrap();
    assert_eq!((second.downloaded, second.reused, second.skipped), (1, 2, 0));
    assert!(third.exists());
    assert!(download(&provider, &cache, &none, 2500, |done, _| done < 1).await.unwrap().is_none());
}

#[tokio::test]
async fn rejected_secrets_ask_for_a_new_one() {
    let server = MockServer::start().await;
    let search = || Mock::given(method("POST")).and(path("/api/search/metadata"));
    search().and(header("x-api-key", "wrong")).respond_with(ResponseTemplate::new(401)).mount(&server).await;
    let assets = json!({"assets": {"items": [{"id": "a1", "type": "IMAGE", "originalFileName": "sea.jpg"}], "nextPage": null}});
    let found = ResponseTemplate::new(200).set_body_json(assets);
    search().and(header("x-api-key", "right")).respond_with(found).mount(&server).await;
    let original = Mock::given(method("GET")).and(path("/api/assets/a1/original"));
    let bytes = ResponseTemplate::new(200).set_body_bytes(vec![1; 10]);
    original.and(header("x-api-key", "right")).respond_with(bytes).mount(&server).await;
    let given: Options = [("server", server.uri()), ("album", "a1b2".into())]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect();
    let options = effective(&kind_of("immich").unwrap(), &given).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let cache = Cache::new(dir.path(), 1 << 20).unwrap();
    let wrong = build("immich", &options, Some("wrong")).unwrap();
    let error = download(wrong.as_ref(), &cache, &HashSet::new(), 1 << 20, |_, _| true).await.err().unwrap();
    assert_eq!(failure(error, true).parts().0, NEEDS_SECRET);
    let right = build("immich", &options, Some("right")).unwrap();
    let fetched =
        download(right.as_ref(), &cache, &HashSet::new(), 1 << 20, |_, _| true).await.unwrap().unwrap();
    assert_eq!((fetched.downloaded, fetched.files[0].1.title.as_str()), (1, "sea.jpg"));
    let plain = wprs_providers::Error::Invalid("refusing to send credentials over plain http".into());
    assert_eq!(failure(plain, true).parts().0, "org.freedesktop.DBus.Error.InvalidArgs");
}

#[test]
fn a_provider_syncs_in_one_job_at_a_time() {
    let syncing = std::sync::Mutex::default();
    let first = claim(&syncing, "daily");
    assert!(first.is_some() && claim(&syncing, "daily").is_none() && claim(&syncing, "photos").is_some());
    drop(first);
    assert!(claim(&syncing, "daily").is_some());
}
