mod common;

use common::{Who, package, start, upload};
use reqwest::{Method, StatusCode};
use serde_json::json;

const DAWN: &str = "org.example.dawn";

#[tokio::test(flavor = "multi_thread")]
async fn votes_are_counted_without_voters_and_featured_packages_are_marked() {
    let server = start(&[("WPRS_AUTO_PUBLISH", "all")]).await;
    wprs_server::init(&server.cfg, "Example wallpapers", false, false).unwrap();
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "user").await;
    let bob = server.user(&admin, "bob", "user").await;
    upload(&server, &ana, &package(server.dir.path(), DAWN, "1.0.0")).await;
    let path = format!("/api/v1/packages/{DAWN}/vote");
    assert_eq!(
        server.json(Method::POST, &path, &Who::Nobody, Some(json!({"vote": "up"}))).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        server.json(Method::POST, &path, &ana, Some(json!({"vote": "sideways"}))).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let missing = "/api/v1/packages/org.example.none/vote";
    assert_eq!(
        server.json(Method::POST, missing, &ana, Some(json!({"vote": "up"}))).await.0,
        StatusCode::NOT_FOUND
    );
    server.json(Method::POST, &path, &ana, Some(json!({"vote": "up"}))).await;
    server.json(Method::POST, &path, &ana, Some(json!({"vote": "up"}))).await;
    let (_, counted) = server.json(Method::POST, &path, &bob, Some(json!({"vote": "down"}))).await;
    assert_eq!(
        (counted["up"].as_i64(), counted["down"].as_i64(), counted["mine"].as_str()),
        (Some(1), Some(1), Some("down"))
    );
    assert!(counted.get("voters").is_none() && !counted.to_string().contains("ana"));
    let (_, catalog) = server.json(Method::GET, "/api/v1/catalog", &Who::Nobody, None).await;
    assert_eq!(catalog["items"][0]["votes"], json!({"up": 1, "down": 1}), "{catalog}");
    let (_, many) = server.json(Method::GET, &format!("/api/v1/votes?ids={DAWN}"), &Who::Nobody, None).await;
    assert_eq!(many["items"][DAWN], json!({"up": 1, "down": 1}));
    let (_, own) = server.json(Method::GET, &format!("/api/v1/votes?ids={DAWN}"), &bob, None).await;
    assert_eq!(own["items"][DAWN], json!({"up": 1, "down": 1, "mine": "down"}));
    let bad = server.json(Method::GET, "/api/v1/votes?ids=Not%20An%20Id", &Who::Nobody, None).await.0;
    assert_eq!(bad, StatusCode::UNPROCESSABLE_ENTITY);
    server.json(Method::POST, &path, &bob, Some(json!({"vote": "none"}))).await;
    let (_, mine) = server.json(Method::GET, &path, &bob, None).await;
    assert_eq!((mine["down"].as_i64(), mine["mine"].as_str()), (Some(0), Some("none")));
    let mut limited = StatusCode::OK;
    for _ in 0..70 {
        limited = server.json(Method::POST, &path, &bob, Some(json!({"vote": "up"}))).await.0;
    }
    assert_eq!(limited, StatusCode::TOO_MANY_REQUESTS);
    let feature = json!({"id": DAWN, "note": "Calm mornings"});
    assert_eq!(
        server.json(Method::POST, "/api/v1/admin/featured", &ana, Some(feature.clone())).await.0,
        StatusCode::FORBIDDEN
    );
    let long = json!({"id": DAWN, "note": "x".repeat(201)});
    assert_eq!(
        server.json(Method::POST, "/api/v1/admin/featured", &admin, Some(long)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, _) = server.json(Method::POST, "/api/v1/admin/featured", &admin, Some(feature)).await;
    assert_eq!(status, StatusCode::OK);
    server.json(Method::POST, "/api/v1/admin/repo/publish", &admin, None).await;
    let (_, picked) = server.json(Method::GET, "/api/v1/catalog?featured=true", &Who::Nobody, None).await;
    assert_eq!(
        (picked["items"][0]["id"].as_str(), picked["items"][0]["featured"].as_bool()),
        (Some(DAWN), Some(true))
    );
    let page = server.call(Method::GET, &format!("/packages/{DAWN}"), &Who::Nobody).send().await.unwrap();
    assert!(page.text().await.unwrap().contains("Featured by this server"));
    let gone = format!("/api/v1/admin/featured/{DAWN}");
    assert_eq!(server.call(Method::DELETE, &gone, &admin).send().await.unwrap().status(), StatusCode::OK);
    let (_, picked) = server.json(Method::GET, "/api/v1/catalog?featured=true", &Who::Nobody, None).await;
    assert!(picked["items"].as_array().unwrap().is_empty());
}
