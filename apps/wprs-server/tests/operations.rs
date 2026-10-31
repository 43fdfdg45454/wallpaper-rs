mod common;

use base64::Engine;
use common::{Who, package, start, upload};
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use std::fs;
use wprs_server::{App, Config};

#[tokio::test(flavor = "multi_thread")]
async fn backup_and_restore_keep_accounts_packages_and_the_signed_index() {
    let server = start(&[("WPRS_AUTO_PUBLISH", "all")]).await;
    wprs_server::init(&server.cfg, "Example wallpapers", false, false).unwrap();
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "user").await;
    let status = upload(&server, &ana, &package(server.dir.path(), "org.example.dawn", "1.0.0")).await;
    assert_eq!(status["state"], "published", "{status}");
    let (desk, _) = server.device("ana").await;
    server.json(Method::POST, "/api/v1/sync/setup", &desk, Some(json!({}))).await;
    let sealed = base64::prelude::BASE64_STANDARD.encode([1u8; 297]);
    let record = json!({"collection": "settings", "item": "playback", "base": 0, "payload": sealed});
    let changes = json!({"epoch": 0, "changes": [record]});
    let (_, synced) = server.json(Method::POST, "/api/v1/sync/changes", &desk, Some(changes)).await;
    assert_eq!(synced["results"][0]["status"], "ok", "{synced}");
    let (_, published) = server.json(Method::POST, "/api/v1/admin/repo/publish", &admin, None).await;
    let before = published["entry_version"].as_u64().unwrap();
    let snapshot = wprs_server::backup(&server.cfg, None).unwrap();
    let manifest: Value = serde_json::from_slice(&fs::read(snapshot.join("manifest.json")).unwrap()).unwrap();
    let files = manifest["files"].as_object().unwrap();
    assert!(
        files.contains_key("wprs.sqlite")
            && files.contains_key("keys/online.key")
            && files.keys().any(|k| k.starts_with("thumbs/"))
    );
    let target = server.dir.path().join("restored");
    let cfg = Config { data: target.clone(), ..server.cfg.clone() };
    wprs_server::restore(&cfg, &snapshot, false).unwrap();
    assert!(wprs_server::restore(&cfg, &snapshot, false).is_err());
    let app = App::open(cfg.clone()).unwrap();
    assert!(app.publish().await.unwrap().unwrap() > before);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(app.serve(listener));
    let http = reqwest::Client::new();
    let item: Value = http
        .get(format!("{url}/api/v1/packages/org.example.dawn"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(item["owner"], "ana");
    let login =
        json!({"handle": "ana", "password": "a long enough password", "device": {"platform": "linux"}});
    assert_eq!(
        http.post(format!("{url}/api/v1/auth/login")).json(&login).send().await.unwrap().status(),
        StatusCode::OK
    );
    let Who::Bearer(token) = desk else { unreachable!() };
    let pull = |since: u64| http.get(format!("{url}/api/v1/sync/changes?since={since}")).bearer_auth(&token);
    assert_eq!(pull(1).send().await.unwrap().status(), StatusCode::GONE);
    let fresh: Value = pull(0).send().await.unwrap().json().await.unwrap();
    assert!(fresh["changes"].as_array().unwrap().len() == 1 && fresh["seq"].as_i64().unwrap() > 1, "{fresh}");
    let latest = http.get(format!("{url}/repo/entry/latest")).send().await.unwrap();
    assert_eq!(latest.headers()["cache-control"], "no-cache");
    let seen = latest.text().await.unwrap().trim().parse::<u64>().unwrap();
    assert!(seen > before);
    wprs_server::restore(&cfg, &snapshot, true).unwrap();
    let kept = fs::read_to_string(target.join("repo/entry/latest")).unwrap();
    assert!(kept.trim() == seen.to_string() && target.join(format!("repo/entry/{seen}.json")).is_file());
    let damaged = snapshot.join("wprs.sqlite");
    fs::write(&damaged, b"broken").unwrap();
    let other = Config { data: server.dir.path().join("other"), ..server.cfg.clone() };
    assert!(wprs_server::restore(&other, &snapshot, false).unwrap_err().to_string().contains("checksum"));
}

#[tokio::test]
async fn invites_users_and_transparency_need_an_admin() {
    let server = start(&[("WPRS_REGISTRATION", "closed")]).await;
    let admin = server.admin().await;
    let closed = json!({"handle": "eve", "password": "a long enough password", "invite": "x"});
    assert_eq!(
        server.json(Method::POST, "/api/v1/auth/register", &Who::Nobody, Some(closed)).await.0,
        StatusCode::FORBIDDEN
    );
    let (_, users) = server.json(Method::GET, "/api/v1/admin/users", &admin, None).await;
    let me = users["items"][0]["id"].as_i64().unwrap();
    let own = server
        .call(Method::PATCH, &format!("/api/v1/admin/users/{me}"), &admin)
        .json(&json!({"role": "user"}))
        .send()
        .await
        .unwrap();
    assert_eq!(own.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let csv = server
        .call(Method::GET, "/api/v1/admin/transparency", &admin)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(csv.starts_with("category,name,count"));
    let page = server.call(Method::GET, "/admin", &admin).send().await.unwrap().text().await.unwrap();
    assert!(page.contains("No repository yet"));
    assert_eq!(
        server.call(Method::GET, "/admin", &Who::Nobody).send().await.unwrap().status(),
        StatusCode::SEE_OTHER
    );
    let (status, problem) = server.json(Method::POST, "/api/v1/admin/repo/publish", &admin, None).await;
    assert_eq!((status, problem["code"].as_str()), (StatusCode::CONFLICT, Some("no_repository")));
}

#[cfg(feature = "oidc")]
#[tokio::test]
async fn single_sign_on_redirects_to_the_issuer_with_pkce() {
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let issuer = MockServer::start().await;
    let base = issuer.uri();
    let metadata = json!({
        "issuer": base, "authorization_endpoint": format!("{base}/authorize"),
        "token_endpoint": format!("{base}/token"), "jwks_uri": format!("{base}/jwks"),
        "response_types_supported": ["code"], "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
    });
    Mock::given(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata))
        .mount(&issuer)
        .await;
    Mock::given(path("/jwks"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"keys": []})))
        .mount(&issuer)
        .await;
    let server = start(&[
        ("WPRS_OIDC_ISSUER", &base),
        ("WPRS_OIDC_CLIENT_ID", "wprs"),
        ("WPRS_OIDC_CLIENT_SECRET", "secret"),
    ])
    .await;
    let response =
        server.call(Method::GET, "/api/v1/auth/oidc/start?next=/account", &Who::Nobody).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response.headers()["location"].to_str().unwrap();
    assert!(
        location.starts_with(&format!("{base}/authorize"))
            && location.contains("code_challenge_method=S256")
            && location.contains("nonce=")
    );
    let requests = issuer.received_requests().await.unwrap();
    assert!(requests.iter().all(|r| r.headers["user-agent"] == "wprs-server"));
    let bad = server
        .call(Method::GET, "/api/v1/auth/oidc/callback?code=x&state=y", &Who::Nobody)
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
