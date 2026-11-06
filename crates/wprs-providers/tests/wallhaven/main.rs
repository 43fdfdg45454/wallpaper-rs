use std::time::Duration;
use wiremock::{MockServer, Request, ResponseTemplate};
use wprs_providers::Wallhaven;
use wprs_providers::wallhaven::{Search, new_seed, option_hint};

mod paging;
mod query;

pub type Pairs = &'static [(&'static str, &'static str)];

pub fn page(count: usize, last: u32, seed: Option<&str>) -> ResponseTemplate {
    let paper = |n: usize| {
        let (id, root) = (format!("p{n:05}"), "https://w.wallhaven.cc/full/p0");
        serde_json::json!({"id": id, "url": format!("https://wallhaven.cc/w/{id}"), "resolution": "1920x1080", "path": format!("{root}/wallhaven-{id}.jpg")})
    };
    let data: Vec<_> = (0..count).map(paper).collect();
    ResponseTemplate::new(200)
        .set_body_json(serde_json::json!({"data": data, "meta": {"last_page": last, "seed": seed}}))
}

pub fn params(pairs: Pairs) -> impl Fn(&Request) -> bool + Send + Sync {
    move |request: &Request| {
        pairs.iter().all(|&pair| request.url.query_pairs().any(|(k, v)| (&*k, &*v) == pair))
    }
}

pub fn quick(server: &MockServer, search: Search) -> Wallhaven {
    let mut wallhaven = Wallhaven::new().unwrap();
    wallhaven.base = format!("{}/api/v1", server.uri());
    (wallhaven.search, wallhaven.window, wallhaven.backoff) =
        (search, Duration::from_millis(100), Duration::from_millis(10));
    wallhaven
}

pub async fn requests(server: &MockServer) -> Vec<String> {
    let all = server.received_requests().await.unwrap();
    all.iter().map(|r| r.url.query().unwrap_or_default().to_owned()).collect()
}

#[test]
fn options_are_checked_and_read_back() {
    assert_eq!(option_hint("ratios", "16x9,landscape,43x18"), None);
    assert!(option_hint("ratios", "wide").is_some());
    assert!(option_hint("colors", "660000,123456").is_some());
    assert!(option_hint("resolutions", "1920x1080,x").is_some());
    assert!(option_hint("collection", "alice/12").is_none() && option_hint("collection", "12").is_some());
    assert!(option_hint("tag", "abc").is_some() && option_hint("similar", "94x38z").is_none());
    assert!(option_hint("query", "anything").is_none());
    let get = |key: &str| match key {
        "colors" => " #660000, FFFFFF ".to_owned(),
        "pages" => "0".to_owned(),
        "sorting" => "hot".to_owned(),
        _ => String::new(),
    };
    let search = Search::from_options(get);
    assert_eq!(
        (search.colors, search.pages, search.sorting.as_str()),
        (vec!["660000".into(), "ffffff".into()], 1, "hot")
    );
    let seed = new_seed();
    assert!(
        seed.len() == 6
            && seed.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
            && seed != new_seed()
    );
}
