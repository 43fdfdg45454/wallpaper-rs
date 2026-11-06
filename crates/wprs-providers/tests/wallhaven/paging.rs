use super::{page, params, quick, requests};
use std::time::{Duration, Instant};
use wiremock::matchers::{method, path, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};
use wprs_providers::wallhaven::Search;
use wprs_providers::{Error, Provider};

#[tokio::test]
async fn too_many_requests_back_off_and_then_give_up() {
    let server = MockServer::start().await;
    let slow = ResponseTemplate::new(429);
    Mock::given(method("GET"))
        .respond_with(slow.clone())
        .up_to_n_times(2)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("GET")).respond_with(page(1, 1, None)).with_priority(2).mount(&server).await;
    let wallhaven = quick(&server, Search::default());
    assert_eq!(wallhaven.list().await.unwrap().len(), 1);
    assert_eq!(requests(&server).await.len(), 3);

    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(slow).mount(&server).await;
    let started = Instant::now();
    let failed = quick(&server, Search::default()).list().await;
    assert!(matches!(failed, Err(Error::Http(e)) if e.status().is_some_and(|s| s.as_u16() == 429)));
    assert_eq!(requests(&server).await.len(), 4);
    assert!(started.elapsed() >= Duration::from_millis(70));
}

#[tokio::test]
async fn pages_share_the_seed_and_stop_at_the_caps() {
    let server = MockServer::start().await;
    let first = Mock::given(path("/api/v1/search")).and(query_param_is_missing("page"));
    first.respond_with(page(24, 5, Some("ABC123"))).mount(&server).await;
    let next = Mock::given(path("/api/v1/search")).and(params(&[("seed", "ABC123")]));
    next.respond_with(page(24, 5, Some("ABC123"))).mount(&server).await;
    let random = Search { sorting: "random".into(), pages: 3, ..Search::default() };
    let items = quick(&server, random.clone()).list().await.unwrap();
    assert_eq!(items.len(), 72);
    let sent = requests(&server).await;
    assert!(
        !sent[0].contains("seed") && sent[1].contains("page=2") && sent[2].contains("page=3"),
        "{sent:?}"
    );

    let capped = Search { max_items: 30, ..random };
    assert_eq!(quick(&server, capped).list().await.unwrap().len(), 30);
    let sent = requests(&server).await;
    assert_eq!(sent.len(), 5);
    assert!(sent[4].contains("seed=ABC123"));
}

#[tokio::test]
async fn paging_ends_at_the_last_page() {
    let server = MockServer::start().await;
    Mock::given(path("/api/v1/search")).respond_with(page(24, 2, None)).mount(&server).await;
    let wallhaven = quick(&server, Search { pages: 10, ..Search::default() });
    assert_eq!(wallhaven.list().await.unwrap().len(), 48);
    assert_eq!(requests(&server).await.len(), 2);
}

#[tokio::test]
async fn sketchy_and_nsfw_need_the_key_before_anything_is_sent() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(page(1, 1, None)).mount(&server).await;
    let mut wallhaven = quick(&server, Search { purity: "110".into(), ..Search::default() });
    assert!(matches!(wallhaven.list().await, Err(Error::Setting(m)) if m.contains("API key")));
    assert!(requests(&server).await.is_empty());
    wallhaven.api_key = Some("secret".into());
    assert_eq!(wallhaven.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn no_more_than_45_requests_go_out_in_a_window() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(page(0, 1, None)).mount(&server).await;
    let mut wallhaven = quick(&server, Search::default());
    wallhaven.window = Duration::from_millis(600);
    let started = Instant::now();
    for _ in 0..46 {
        wallhaven.list().await.unwrap();
    }
    assert!(started.elapsed() >= Duration::from_millis(550), "{:?}", started.elapsed());
}
