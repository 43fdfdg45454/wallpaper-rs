mod common;

use common::{Who, content_type, start};
use reqwest::{Method, StatusCode};

#[tokio::test]
async fn discovery_health_and_legal_pages() {
    let server =
        start(&[("WPRS_LEGAL_OPERATOR", "Example Wallpapers"), ("WPRS_LEGAL_CONTACT", "abuse@example.org")])
            .await;
    assert_eq!(
        server.call(Method::GET, "/healthz", &Who::Nobody).send().await.unwrap().text().await.unwrap(),
        "ok"
    );
    let (status, ready) = server.json(Method::GET, "/readyz", &Who::Nobody, None).await;
    assert_eq!((status, ready["database"].as_bool()), (StatusCode::OK, Some(true)));
    let (_, info) = server.json(Method::GET, "/.well-known/wprs-server", &Who::Nobody, None).await;
    assert_eq!(
        (info["sync"]["end_to_end_default"].as_bool(), info["registration"].as_str()),
        (Some(true), Some("invite"))
    );
    let legal = server.call(Method::GET, "/legal", &Who::Nobody).send().await.unwrap().text().await.unwrap();
    assert!(legal.contains("Example Wallpapers") && legal.contains("abuse@example.org"));
    for doc in ["terms", "privacy", "content", "notice", "counter-notice"] {
        let page = server.call(Method::GET, &format!("/legal/{doc}"), &Who::Nobody).send().await.unwrap();
        assert_eq!((doc, page.status()), (doc, StatusCode::OK));
        assert!(!page.text().await.unwrap().contains("<script"));
    }
    let missing = server.call(Method::GET, "/nowhere", &Who::Nobody).send().await.unwrap();
    assert_eq!(
        (missing.status(), content_type(&missing).starts_with("text/html")),
        (StatusCode::NOT_FOUND, true)
    );
    let (status, problem) =
        server.json(Method::GET, "/api/v1/packages/org.example.none", &Who::Nobody, None).await;
    assert_eq!((status, problem["code"].as_str()), (StatusCode::NOT_FOUND, Some("not_found")));
}
