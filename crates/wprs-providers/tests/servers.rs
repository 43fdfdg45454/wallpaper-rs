use serde_json::json;
use wiremock::matchers::{basic_auth, body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wprs_providers::{Cache, Immich, Item, Media, Provider, WebDav};

const ALBUM: &str = "5c6d7e8f-9a0b-4c1d-8e2f-3a4b5c6d7e8f";
const ASSET: &str = "8d2f1c3a-5b6e-4f70-8a91-b2c3d4e5f601";
const LAST_PAGE: &str = r#"{"albums":{"total":0,"count":0,"items":[],"facets":[]},
    "assets":{"total":3,"count":0,"items":[],"facets":[],"nextCursor":null,"nextPage":null}}"#;

fn bytes(body: &[u8]) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_bytes(body.to_vec())
}

#[test]
fn nextcloud_urls_encode_user_and_folder() {
    let dav =
        WebDav::nextcloud("https://cloud.example.com/", "alice@example.com", "x", "/My Walls/").unwrap();
    assert_eq!(dav.url, "https://cloud.example.com/remote.php/dav/files/alice%40example.com/My%20Walls/");
}

#[tokio::test]
async fn webdav_lists_one_level_and_downloads_with_basic_auth() {
    let server = MockServer::start().await;
    let folder = Mock::given(method("PROPFIND")).and(path("/remote.php/dav/files/alice/Wallpapers/"));
    let folder = folder.and(header("Depth", "1")).and(basic_auth("alice", "app-password"));
    let listing =
        ResponseTemplate::new(207).set_body_raw(include_str!("fixtures/webdav.xml"), "application/xml");
    folder.respond_with(listing).expect(1).mount(&server).await;
    let file =
        Mock::given(method("GET")).and(path("/remote.php/dav/files/alice/Wallpapers/Mountain%20Lake.jpg"));
    file.and(basic_auth("alice", "app-password")).respond_with(bytes(b"lake")).expect(1).mount(&server).await;

    let dav = WebDav::nextcloud(&server.uri(), "alice", "app-password", "Wallpapers").unwrap();
    let items = dav.list().await.unwrap();
    let found: Vec<_> = items.iter().map(|item| (item.title.as_str(), item.media)).collect();
    assert_eq!(found, [("Mountain Lake.jpg", Media::Image), ("Waves.webm", Media::Video)]);
    assert_eq!(
        items[0].url,
        format!("{}/remote.php/dav/files/alice/Wallpapers/Mountain%20Lake.jpg", server.uri())
    );

    let dir = tempfile::tempdir().unwrap();
    let stored = dav.fetch(&items[0], &Cache::new(dir.path(), 1 << 20).unwrap()).await.unwrap();
    assert_eq!(std::fs::read(stored).unwrap(), b"lake");
}

#[tokio::test]
async fn immich_pages_through_an_album_and_downloads_originals() {
    let server = MockServer::start().await;
    let search =
        || Mock::given(method("POST")).and(path("/api/search/metadata")).and(header("x-api-key", "key"));
    let first = search().and(body_partial_json(json!({ "albumIds": [ALBUM], "page": 1 })));
    first
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(include_str!("fixtures/immich.json"), "application/json"),
        )
        .expect(1)
        .mount(&server)
        .await;
    let second = search().and(body_partial_json(json!({ "albumIds": [ALBUM], "page": 2 })));
    second
        .respond_with(ResponseTemplate::new(200).set_body_raw(LAST_PAGE, "application/json"))
        .expect(1)
        .mount(&server)
        .await;
    let original = Mock::given(method("GET")).and(path(format!("/api/assets/{ASSET}/original")));
    original.and(header("x-api-key", "key")).respond_with(bytes(b"beach")).expect(1).mount(&server).await;

    let immich = Immich::new(&format!("{}/", server.uri()), "key", ALBUM).unwrap();
    let items = immich.list().await.unwrap();
    let found: Vec<_> = items.iter().map(|item| (item.title.as_str(), item.media)).collect();
    assert_eq!(found, [("Beach.jpg", Media::Image), ("Waves.mp4", Media::Video)]);
    assert_eq!(items[0].id, format!("immich-{ASSET}"));

    let dir = tempfile::tempdir().unwrap();
    let stored = immich.fetch(&items[0], &Cache::new(dir.path(), 1 << 20).unwrap()).await.unwrap();
    assert_eq!(
        (stored.extension().unwrap().to_str(), std::fs::read(&stored).unwrap()),
        (Some("jpg"), b"beach".to_vec())
    );
}

#[tokio::test]
async fn credentials_stay_on_their_origin_and_off_plain_http() {
    let (server, other) = (MockServer::start().await, MockServer::start().await);
    let moved = ResponseTemplate::new(302).insert_header("location", format!("{}/leak", other.uri()));
    Mock::given(method("GET")).respond_with(moved).mount(&server).await;
    Mock::given(method("GET")).respond_with(bytes(b"leak")).expect(0).mount(&other).await;
    let immich = Immich::new(&server.uri(), "key", ALBUM).unwrap();
    let url = format!("{}/api/assets/{ASSET}/original", server.uri());
    let item = Item {
        id: ASSET.into(),
        title: "a.jpg".into(),
        url,
        attribution: None,
        license: None,
        media: Media::Image,
    };
    let dir = tempfile::tempdir().unwrap();
    assert!(immich.fetch(&item, &Cache::new(dir.path(), 1 << 20).unwrap()).await.is_err());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);

    let remote = Immich::new("http://photos.example.org", "key", ALBUM).unwrap();
    assert!(remote.list().await.is_err());
    let dav = WebDav::new("http://dav.example.org/walls/", "alice", "x").unwrap();
    assert!(dav.list().await.is_err());
}
