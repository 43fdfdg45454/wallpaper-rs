use super::{Pairs, page, params, quick, requests};
use wiremock::matchers::{header, path, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wprs_providers::wallhaven::Search;
use wprs_providers::{Error, Provider};

#[tokio::test]
async fn every_option_reaches_the_query_string() {
    let server = MockServer::start().await;
    let expected: Pairs = &[
        ("categories", "101"),
        ("purity", "100"),
        ("sorting", "toplist"),
        ("order", "asc"),
        ("topRange", "3M"),
        ("atleast", "1920x1080"),
        ("resolutions", "1920x1080,2560x1440"),
        ("ratios", "16x9,landscape"),
        ("colors", "660000,ffffff"),
        ("q", "mountains @alice"),
        ("page", "3"),
    ];
    let search =
        Mock::given(path("/api/v1/search")).and(params(expected)).and(query_param_is_missing("seed"));
    search.respond_with(page(2, 9, None)).expect(1).mount(&server).await;
    let search = Search {
        categories: "101".into(),
        order: "asc".into(),
        top_range: "3M".into(),
        atleast: Some("1920x1080".into()),
        resolutions: vec!["1920x1080".into(), "2560x1440".into()],
        ratios: vec!["16x9".into(), "landscape".into()],
        colors: vec!["660000".into(), "ffffff".into()],
        query: Some("mountains".into()),
        uploader: Some("alice".into()),
        page: 3,
        ..Search::default()
    };
    assert_eq!(quick(&server, search).list().await.unwrap().len(), 2);
}

#[tokio::test]
async fn the_range_and_the_seed_go_only_with_their_sorting() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/search")).respond_with(page(1, 1, Some("ABC123"))).mount(&server).await;
    let recent = Search { sorting: "date_added".into(), seed: Some("ABC123".into()), ..Search::default() };
    quick(&server, recent).list().await.unwrap();
    let random = Search { sorting: "random".into(), seed: Some("ABC123".into()), ..Search::default() };
    quick(&server, random).list().await.unwrap();
    let hot = Search { sorting: "hot".into(), ..Search::default() };
    quick(&server, hot).list().await.unwrap();
    let sent = requests(&server).await;
    assert!(!sent[0].contains("topRange") && !sent[0].contains("seed"), "{}", sent[0]);
    assert!(sent[1].contains("seed=ABC123") && !sent[1].contains("topRange"), "{}", sent[1]);
    assert!(sent[2].contains("sorting=hot"), "{}", sent[2]);
}

#[tokio::test]
async fn similar_tag_and_user_searches_use_the_search_terms() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/search")).respond_with(page(1, 1, None)).mount(&server).await;
    for (search, term) in [
        (Search { similar: Some("94x38z".into()), ..Search::default() }, "q=like%3A94x38z"),
        (Search { tag: Some("1".into()), ..Search::default() }, "q=id%3A1"),
        (Search { uploader: Some("alice".into()), ..Search::default() }, "q=%40alice"),
    ] {
        quick(&server, search).list().await.unwrap();
        assert!(requests(&server).await.last().unwrap().contains(term), "{term}");
    }
    let mixed = Search { similar: Some("94x38z".into()), query: Some("sea".into()), ..Search::default() };
    assert!(matches!(quick(&server, mixed).list().await, Err(Error::Setting(_))));
    assert_eq!(requests(&server).await.len(), 3);
}

#[tokio::test]
async fn collections_are_listed_and_read_with_the_purity_filter_only() {
    let server = MockServer::start().await;
    let list = serde_json::json!({"data": [{"id": 7, "label": "Tall boys", "views": 3, "public": 1, "count": 32}, {"id": 9, "label": "Mine", "views": 0, "public": 0, "count": 1}]});
    let own = Mock::given(path("/api/v1/collections")).and(header("X-API-Key", "secret"));
    own.respond_with(ResponseTemplate::new(200).set_body_json(&list)).expect(1).mount(&server).await;
    let theirs = Mock::given(path("/api/v1/collections/alice"));
    theirs.respond_with(ResponseTemplate::new(200).set_body_json(&list)).expect(1).mount(&server).await;
    let read =
        Mock::given(path("/api/v1/collections/alice/7")).and(params(&[("purity", "100"), ("page", "2")]));
    let read = read.and(query_param_is_missing("sorting")).and(query_param_is_missing("categories"));
    read.respond_with(page(3, 4, None)).expect(1).mount(&server).await;

    let mut wallhaven =
        quick(&server, Search { collection: Some("alice/7".into()), page: 2, ..Search::default() });
    assert_eq!(wallhaven.list().await.unwrap().len(), 3);
    let public = wallhaven.collections(Some("alice")).await.unwrap();
    assert_eq!(
        (public[0].id, public[0].label.as_str(), public[0].count, public[0].public),
        (7, "Tall boys", 32, true)
    );
    assert!(wallhaven.collections(None).await.is_err());
    wallhaven.api_key = Some("secret".into());
    let own = wallhaven.collections(None).await.unwrap();
    assert_eq!(own.len(), 2);
    assert!(!own[1].public);
}
