mod common;

use common::{HttpFetch, Who, package, start};
use reqwest::multipart::{Form, Part};
use reqwest::{Method, StatusCode};
use serde_json::json;
use std::fs;
use wprs_format::{add_repo, browse, install};

const NOON: &str = "org.example.noon";

#[tokio::test(flavor = "multi_thread")]
async fn unlisted_uploads_stay_out_of_the_catalog_but_keep_their_link() {
    let server = start(&[("WPRS_AUTO_PUBLISH", "all")]).await;
    wprs_server::init(&server.cfg, "Example wallpapers", false, false).unwrap();
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "user").await;
    let wpak = package(server.dir.path(), NOON, "1.0.0");
    let bytes = fs::read(&wpak).unwrap();
    let sha = wprs_format::sha256(&bytes[..]).unwrap();
    let bad = json!({"size": bytes.len(), "sha256": sha, "visibility": "friends"});
    assert_eq!(
        server.json(Method::POST, "/api/v1/uploads", &ana, Some(bad)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let part = Part::bytes(bytes).file_name("noon.wpak");
    let form = Form::new().text("visibility", "unlisted").part("package", part);
    let page = server.call(Method::POST, "/upload?back=/account", &ana).multipart(form).send().await.unwrap();
    assert_eq!(page.status(), StatusCode::OK);
    server.json(Method::POST, "/api/v1/admin/repo/publish", &admin, None).await;
    let (_, catalog) = server.json(Method::GET, "/api/v1/catalog?q=noon", &Who::Nobody, None).await;
    assert!(catalog["items"].as_array().unwrap().is_empty(), "{catalog}");
    let (status, item) =
        server.json(Method::GET, &format!("/api/v1/packages/{NOON}"), &Who::Nobody, None).await;
    assert_eq!((status, item["listed"].as_bool()), (StatusCode::OK, Some(false)), "{item}");
    let shown = server.call(Method::GET, &format!("/packages/{NOON}"), &Who::Nobody).send().await.unwrap();
    assert!(shown.text().await.unwrap().contains("Unlisted"));
    let (_, mine) = server.json(Method::GET, "/api/v1/me/packages", &ana, None).await;
    assert_eq!(mine["items"][0]["visibility"], "unlisted", "{mine}");
    let url = wprs_server::status(&server.cfg).unwrap()["url"].as_str().unwrap().to_string();
    let data = server.dir.path().join("client");
    let client = data.clone();
    let found = tokio::task::spawn_blocking(move || {
        add_repo(&client, &HttpFetch, &url, now()).unwrap();
        let hidden = browse(&client, "").unwrap().is_empty();
        install(&client, &HttpFetch, NOON, now(), false, false).unwrap();
        hidden
    });
    assert!(found.await.unwrap(), "an unlisted package was browsed");
    let path = format!("/api/v1/me/packages/{NOON}/visibility");
    let bob = server.user(&admin, "bob", "user").await;
    let public = json!({"visibility": "public"});
    assert_eq!(server.json(Method::POST, &path, &bob, Some(public.clone())).await.0, StatusCode::NOT_FOUND);
    let wrong = json!({"visibility": "secret"});
    assert_eq!(server.json(Method::POST, &path, &ana, Some(wrong)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let missing = json!({"visibility": "public", "version": "9.0.0"});
    assert_eq!(server.json(Method::POST, &path, &ana, Some(missing)).await.0, StatusCode::NOT_FOUND);
    let (status, set) = server.json(Method::POST, &path, &ana, Some(public)).await;
    assert_eq!((status, set["visibility"].as_str()), (StatusCode::OK, Some("public")), "{set}");
    server.json(Method::POST, "/api/v1/admin/repo/publish", &admin, None).await;
    let (_, catalog) = server.json(Method::GET, "/api/v1/catalog?q=noon", &Who::Nobody, None).await;
    assert_eq!(catalog["items"][0]["id"], NOON, "{catalog}");
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}
