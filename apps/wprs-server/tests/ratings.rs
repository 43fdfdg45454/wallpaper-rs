mod common;

use common::{HttpFetch, Who, package, package_v2, start, upload};
use reqwest::{Method, StatusCode};
use serde_json::json;
use wprs_format::{Rating, add_repo, browse};

#[tokio::test(flavor = "multi_thread")]
async fn mature_uploads_are_refused_and_the_catalog_filters_by_rating_and_facet() {
    let server = start(&[]).await;
    wprs_server::init(&server.cfg, "Example wallpapers", false).unwrap();
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "user").await;
    let refused =
        upload(&server, &ana, &package_v2(server.dir.path(), "org.example.night", "1.0.0", Rating::Mature))
            .await;
    assert_eq!(refused["state"], "failed", "{refused}");
    assert!(refused["errors"][0].as_str().unwrap().contains("does not accept mature content"), "{refused}");
    upload(&server, &ana, &package_v2(server.dir.path(), "org.example.dusk", "1.0.0", Rating::General)).await;
    upload(&server, &ana, &package(server.dir.path(), "org.example.dawn", "1.0.0")).await;
    let (_, queue) = server.json(Method::GET, "/api/v1/mod/queue", &admin, None).await;
    for case in queue["items"].as_array().unwrap().iter().map(|c| c["id"].as_i64().unwrap()) {
        let path = format!("/api/v1/mod/cases/{case}/decision");
        let (status, _) =
            server.json(Method::POST, &path, &admin, Some(json!({"decision": "approve"}))).await;
        assert_eq!(status, StatusCode::OK);
    }
    let ids = |query: &'static str| {
        let server = &server;
        async move {
            let (_, found) =
                server.json(Method::GET, &format!("/api/v1/catalog{query}"), &Who::Nobody, None).await;
            found["items"]
                .as_array()
                .unwrap()
                .iter()
                .map(|i| i["id"].as_str().unwrap().to_string())
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(ids("").await, ["org.example.dawn", "org.example.dusk"]);
    assert_eq!(ids("?content=general").await, ["org.example.dusk"]);
    assert_eq!(ids("?content=unrated").await, ["org.example.dawn"]);
    assert_eq!(ids("?facet=photosensitive").await, ["org.example.dusk"]);
    assert!(ids("?content=mature").await.is_empty());
    let (_, dusk) = server.json(Method::GET, "/api/v1/packages/org.example.dusk", &Who::Nobody, None).await;
    assert_eq!((&dusk["content"], &dusk["photosensitive"]), (&json!("general"), &json!(true)), "{dusk}");
    assert_eq!(dusk["facets"], json!(["android", "landscape", "photosensitive"]));
    let page = server.call(Method::GET, "/packages/org.example.dusk", &Who::Nobody).send().await.unwrap();
    assert!(page.text().await.unwrap().contains("flashing lights"));
    let (status, _) = server.json(Method::POST, "/api/v1/admin/repo/publish", &admin, None).await;
    assert_eq!(status, StatusCode::OK);
    let url = wprs_server::status(&server.cfg).unwrap()["url"].as_str().unwrap().to_string();
    let data = server.dir.path().join("client");
    let found = tokio::task::spawn_blocking(move || {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        add_repo(&data, &HttpFetch, &url, now as i64).unwrap();
        browse(&data, "").unwrap()
    });
    let found = found.await.unwrap();
    let rated: Vec<_> = found.iter().map(|i| (i.id.as_str(), i.release.photosensitive)).collect();
    assert_eq!(rated, [("org.example.dawn", None), ("org.example.dusk", Some(true))]);
}
