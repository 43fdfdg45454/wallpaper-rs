use wiremock::matchers::{header, path, query_param_is_missing};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};
use wprs_providers::{Apod, Bing, Cache, Item, Media, MotionBgs, Provider, Wallhaven, WikimediaPotd, client};

type Pairs = &'static [(&'static str, &'static str)];

fn json(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body, "application/json")
}

fn params(pairs: Pairs) -> impl Fn(&Request) -> bool + Send + Sync {
    move |request: &Request| {
        pairs.iter().all(|&pair| request.url.query_pairs().any(|(k, v)| (&*k, &*v) == pair))
    }
}

fn lines(key: &'static str, count: usize) -> impl Fn(&Request) -> bool + Send + Sync {
    move |request: &Request| request.url.query_pairs().any(|(k, v)| k == key && v.lines().count() == count)
}

#[tokio::test]
async fn bing_lists_daily_images_and_fetches_the_uhd_variant() {
    let server = MockServer::start().await;
    let archive = params(&[("format", "js"), ("idx", "0"), ("n", "8"), ("mkt", "de-DE")]);
    let agent = header("user-agent", concat!("wallpaper-rs/", env!("CARGO_PKG_VERSION")));
    let archive = Mock::given(path("/HPImageArchive.aspx")).and(archive).and(agent);
    archive.respond_with(json(include_str!("fixtures/bing.json"))).expect(2).mount(&server).await;
    let image = Mock::given(path("/th")).and(params(&[("id", "OHR.AutumnFjord_EN-US4385123890_UHD.jpg")]));
    image.respond_with(ResponseTemplate::new(200).set_body_bytes(*b"jpeg")).expect(1).mount(&server).await;

    let bing = Bing { base: server.uri(), ..Bing::new(client().unwrap(), "de-DE", true) };
    let items = bing.list().await.unwrap();
    assert_eq!(items[0].id, "bing-OHR.AutumnFjord_EN-US4385123890_UHD");
    assert_eq!(items[0].url, format!("{}/th?id=OHR.AutumnFjord_EN-US4385123890_UHD.jpg", server.uri()));
    assert_eq!(items[0].title, "Autumn on the fjord");
    assert_eq!(Some(items[1].title.as_str()), items[1].attribution.as_deref());

    let full_hd = Bing { uhd: false, ..bing.clone() }.list().await.unwrap();
    assert!(full_hd[0].url.ends_with("_1920x1080.jpg&rf=LaDigue_1920x1080.jpg&pid=hp"));

    let dir = tempfile::tempdir().unwrap();
    let path = bing.fetch(&items[0], &Cache::new(dir.path(), 1 << 20).unwrap()).await.unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"jpeg");
}

#[tokio::test]
async fn apod_returns_images_and_direct_videos_only_when_enabled() {
    let server = MockServer::start().await;
    let dated =
        |request: &Request| request.url.query_pairs().any(|(k, v)| k == "start_date" && v.len() == 10);
    let entries = Mock::given(path("/planetary/apod")).and(params(&[("api_key", "DEMO_KEY")])).and(dated);
    entries.respond_with(json(include_str!("fixtures/apod.json"))).expect(2).mount(&server).await;

    let base = format!("{}/planetary/apod", server.uri());
    let apod = Apod { base, ..Apod::new(client().unwrap(), "DEMO_KEY") };
    let images = apod.list().await.unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(
        (images[0].id.as_str(), images[0].title.as_str()),
        ("apod-2026-10-19", "The Butterfly Nebula")
    );
    assert!(
        images[0].url.starts_with("https://assets.science.nasa.gov/") && images[0].url.contains(".jpg?w=")
    );
    assert_eq!(images[0].attribution.as_deref(), Some("Image Credit & Copyright: Jane Example"));

    let all = Apod { videos: true, ..apod }.list().await.unwrap();
    assert_eq!(all.iter().map(|item| item.media).collect::<Vec<_>>(), [Media::Image, Media::Video]);
    assert_eq!(all[1].url, "https://apod.nasa.gov/apod/image/2610/AuroraTimelapse.mp4");
}

#[tokio::test]
async fn wikimedia_resolves_pictures_of_the_day_with_license_and_artist() {
    let server = MockServer::start().await;
    let expand = params(&[("action", "expandtemplates"), ("prop", "wikitext"), ("format", "json")]);
    let expand = Mock::given(path("/w/api.php")).and(expand).and(lines("text", 2));
    expand.respond_with(json(include_str!("fixtures/potd-expand.json"))).expect(1).mount(&server).await;
    let info =
        params(&[("action", "query"), ("titles", "File:Lake Bled at sunrise.jpg"), ("iiurlwidth", "3840")]);
    let info = Mock::given(path("/w/api.php")).and(info);
    info.respond_with(json(include_str!("fixtures/potd-info.json"))).expect(1).mount(&server).await;

    let base = format!("{}/w/api.php", server.uri());
    let potd = WikimediaPotd { base, days: 2, ..WikimediaPotd::new(client().unwrap()) };
    let file = "a/ab/Lake_Bled_at_sunrise.jpg/3840px-Lake_Bled_at_sunrise.jpg";
    let expected = Item {
        id: "potd-Lake Bled at sunrise.jpg".into(),
        title: "Lake Bled at sunrise".into(),
        url: format!("https://upload.wikimedia.org/wikipedia/commons/thumb/{file}"),
        attribution: Some("Jane Example".into()),
        license: Some("CC BY-SA 4.0".into()),
        media: Media::Image,
    };
    assert_eq!(potd.list().await.unwrap(), [expected]);
}

#[tokio::test]
async fn wallhaven_searches_with_filters_and_sends_the_key_as_a_header() {
    let server = MockServer::start().await;
    let filters: Pairs =
        &[("q", "mountains"), ("categories", "010"), ("purity", "110"), ("sorting", "toplist")];
    let search =
        Mock::given(path("/api/v1/search")).and(params(filters)).and(params(&[("atleast", "2560x1440")]));
    let search = search.and(header("X-API-Key", "secret")).and(query_param_is_missing("apikey"));
    search.respond_with(json(include_str!("fixtures/wallhaven.json"))).expect(1).mount(&server).await;

    let mut wallhaven = Wallhaven::new().unwrap();
    wallhaven.base = format!("{}/api/v1", server.uri());
    (wallhaven.categories, wallhaven.purity) = ("010".into(), "110".into());
    (wallhaven.query, wallhaven.atleast) = (Some("mountains".into()), Some("2560x1440".into()));
    wallhaven.api_key = Some("secret".into());
    let items = wallhaven.list().await.unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].id, "wallhaven-94x38z");
    assert_eq!(items[0].url, "https://w.wallhaven.cc/full/94/wallhaven-94x38z.jpg");
    assert_eq!(items[1].attribution.as_deref(), Some("https://wallhaven.cc/w/ze1p56"));
}

#[tokio::test]
async fn motionbgs_pages_through_a_tag_and_downloads_the_best_available_video() {
    let server = MockServer::start().await;
    let html = |body: &str| ResponseTemplate::new(200).set_body_raw(body, "text/html; charset=utf-8");
    let first = Mock::given(path("/tag:rain/")).respond_with(html(include_str!("fixtures/motionbgs.html")));
    first.expect(2).mount(&server).await;
    let second =
        Mock::given(path("/tag:rain/2/")).respond_with(html(include_str!("fixtures/motionbgs-2.html")));
    second.expect(1).mount(&server).await;
    Mock::given(path("/tag:rain/3/")).respond_with(ResponseTemplate::new(404)).expect(1).mount(&server).await;
    let video =
        Mock::given(path("/dl/4k/10239/")).respond_with(ResponseTemplate::new(200).set_body_bytes(*b"mp4"));
    video.expect(1).mount(&server).await;

    let motion = MotionBgs {
        base: server.uri(),
        tag: Some("rain".into()),
        pages: 5,
        ..MotionBgs::new(client().unwrap())
    };
    let items = motion.list().await.unwrap();
    let found: Vec<_> = items.iter().map(|item| (item.id.as_str(), item.title.as_str())).collect();
    assert_eq!(
        found,
        [
            ("motionbgs-10239", "Rainy Night Ride"),
            ("motionbgs-599", "Black Lamborghini in the Rain"),
            ("motionbgs-9951", "Walter & Jesse in the Rain"),
            ("motionbgs-9963", "Tokyo Midnight Rain"),
        ]
    );
    assert!(items.iter().all(|item| item.media == Media::Video));
    assert_eq!(items[0].url, format!("{}/dl/4k/10239/", server.uri()));
    assert_eq!(items[1].url, format!("{}/dl/hd/599/", server.uri()));
    assert_eq!(items[0].attribution, Some(format!("{}/rainy-night-ride", server.uri())));

    let hd = MotionBgs { uhd: false, pages: 1, ..motion.clone() }.list().await;
    assert!(hd.unwrap().iter().all(|item| item.url.contains("/dl/hd/")));

    let dir = tempfile::tempdir().unwrap();
    let stored = motion.fetch(&items[0], &Cache::new(dir.path(), 1 << 20).unwrap()).await.unwrap();
    assert_eq!(
        (stored.extension().unwrap().to_str(), std::fs::read(&stored).unwrap()),
        (Some("mp4"), b"mp4".to_vec())
    );
}

#[tokio::test]
async fn motionbgs_lists_the_4k_page_without_a_tag() {
    let server = MockServer::start().await;
    let page =
        ResponseTemplate::new(200).set_body_raw(include_str!("fixtures/motionbgs-2.html"), "text/html");
    Mock::given(path("/4k/")).respond_with(page).expect(1).mount(&server).await;
    let motion = MotionBgs { base: server.uri(), ..MotionBgs::new(client().unwrap()) };
    assert_eq!(motion.list().await.unwrap().len(), 2);
}
