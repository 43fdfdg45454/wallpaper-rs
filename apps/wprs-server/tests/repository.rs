mod common;

use common::{HttpFetch, Who, content_type, package, start, upload};
use reqwest::multipart::{Form, Part};
use reqwest::{Method, StatusCode};
use serde_json::json;
use std::fs;
use wprs_format::{Reason, add_repo, install, installed, keygen, sign_source, update_repo};

const DAWN: &str = "org.example.dawn";

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

#[tokio::test(flavor = "multi_thread")]
async fn upload_review_publish_and_install_with_a_verifying_client() {
    let server = start(&[]).await;
    let root = wprs_server::init(&server.cfg, "Example wallpapers", false, false).unwrap();
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "user").await;
    let wpak = package(server.dir.path(), DAWN, "1.0.0");
    let status = upload(&server, &ana, &wpak).await;
    assert_eq!(
        (status["state"].as_str(), status["package"].as_str()),
        (Some("pending_review"), Some(DAWN)),
        "{status}"
    );
    let (_, catalog) = server.json(Method::GET, "/api/v1/catalog", &Who::Nobody, None).await;
    assert!(catalog["items"].as_array().unwrap().is_empty());
    let (status, queue) = server.json(Method::GET, "/api/v1/mod/queue", &admin, None).await;
    assert_eq!(status, StatusCode::OK);
    let case = queue["items"][0]["id"].as_i64().unwrap();
    assert_eq!(server.json(Method::GET, "/api/v1/mod/queue", &ana, None).await.0, StatusCode::FORBIDDEN);
    let thumb = server
        .call(
            Method::GET,
            &format!("/media/{}.jpg", queue["items"][0]["sha256"].as_str().unwrap()),
            &Who::Nobody,
        )
        .send()
        .await
        .unwrap();
    assert_eq!((thumb.status(), content_type(&thumb)), (StatusCode::OK, "image/jpeg".to_string()));
    let mod_page = server.call(Method::GET, "/mod", &admin).send().await.unwrap().text().await.unwrap();
    assert!(mod_page.contains("Dawn 1.0.0"));
    let decision = json!({"decision": "approve"});
    let (status, decided) = server
        .json(Method::POST, &format!("/api/v1/mod/cases/{case}/decision"), &admin, Some(decision))
        .await;
    assert_eq!((status, decided["state"].as_str()), (StatusCode::OK, Some("published")), "{decided}");
    let (status, published) = server.json(Method::POST, "/api/v1/admin/repo/publish", &admin, None).await;
    assert_eq!(status, StatusCode::OK, "{published}");
    let (_, catalog) = server.json(Method::GET, "/api/v1/catalog?q=dawn", &Who::Nobody, None).await;
    assert_eq!(catalog["items"][0]["id"], DAWN);
    let home = server.call(Method::GET, "/?q=Dawn", &Who::Nobody).send().await.unwrap().text().await.unwrap();
    assert!(home.contains("Dawn 1.0.0"));
    let info = wprs_server::status(&server.cfg).unwrap();
    let url = info["url"].as_str().unwrap().to_string();
    assert!(url.contains(&root.fingerprint));
    let data = server.dir.path().join("client");
    let (client, check) = (data.clone(), url.clone());
    let added = tokio::task::spawn_blocking(move || add_repo(&client, &HttpFetch, &check, now()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!((added.packages, added.stale), (1, false));
    let client = data.clone();
    let done = tokio::task::spawn_blocking(move || install(&client, &HttpFetch, DAWN, now(), false, false))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fs::read(&done.path).unwrap(), fs::read(&wpak).unwrap());
    let dupe = upload(&server, &ana, &wpak).await;
    assert!(dupe["errors"][0].as_str().unwrap().contains("newer than 1.0.0"), "{dupe}");
    let bob = server.user(&admin, "bob", "trusted").await;
    let stolen = upload(&server, &bob, &package(server.dir.path(), DAWN, "2.0.0")).await;
    assert!(stolen["errors"][0].as_str().unwrap().contains("another account"), "{stolen}");
    let preview = wprs_format::sha256_file(server.dir.path().join(format!("src-{DAWN}-1.0.0/preview.png")));
    let preview = format!("/repo/previews/{}.png", preview.unwrap());
    let shown = server.call(Method::GET, &preview, &Who::Nobody).send().await.unwrap();
    assert_eq!(shown.status(), StatusCode::OK);
    let report = json!({"category": "malware", "details": "hidden payload"});
    assert_eq!(
        server
            .json(Method::POST, &format!("/api/v1/packages/{DAWN}/reports"), &Who::Nobody, Some(report))
            .await
            .0,
        StatusCode::CREATED
    );
    let (_, queue) = server.json(Method::GET, "/api/v1/mod/queue", &admin, None).await;
    let case = queue["items"][0]["id"].as_i64().unwrap();
    let short = json!({"decision": "revoke", "statement": "bad"});
    assert_eq!(
        server.json(Method::POST, &format!("/api/v1/mod/cases/{case}/decision"), &admin, Some(short)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let statement = "The package hides a payload.";
    let revoke = json!({"decision": "revoke", "statement": statement, "reason": "malware", "strike": true});
    assert_eq!(
        server
            .json(Method::POST, &format!("/api/v1/mod/cases/{case}/decision"), &admin, Some(revoke))
            .await
            .0,
        StatusCode::OK
    );
    server.json(Method::POST, "/api/v1/admin/repo/publish", &admin, None).await;
    let gone = server
        .call(Method::GET, &format!("/repo/packages/{DAWN}/1.0.0.wpak"), &Who::Nobody)
        .send()
        .await
        .unwrap();
    assert_eq!(gone.status(), StatusCode::GONE);
    let hidden = server.call(Method::GET, &preview, &Who::Nobody).send().await.unwrap();
    assert_eq!(hidden.status(), StatusCode::NOT_FOUND);
    let (client, repo_id) = (data.clone(), added.repo_id.clone());
    let outcome = tokio::task::spawn_blocking(move || update_repo(&client, &HttpFetch, &repo_id, now()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(outcome.revoked[0].revoked, Some(Reason::Malware));
    assert_eq!(installed(&data).unwrap()[0].revoked, Some(Reason::Malware));
    let (_, mine) = server.json(Method::GET, "/api/v1/me/packages", &ana, None).await;
    assert_eq!(mine["items"][0]["statement"], "The package hides a payload.");
}

#[tokio::test(flavor = "multi_thread")]
async fn signatures_blocklist_and_web_uploads() {
    let server = start(&[("WPRS_AUTO_PUBLISH", "trusted")]).await;
    wprs_server::init(&server.cfg, "Example wallpapers", false, false).unwrap();
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "trusted").await;
    let key = keygen(None).unwrap();
    let (_, added) = server
        .json(Method::POST, "/api/v1/me/author-keys", &ana, Some(json!({"public_key": key.public})))
        .await;
    assert_eq!(added["fingerprint"].as_str(), Some(key.fingerprint.as_str()));
    let wpak = package(server.dir.path(), DAWN, "1.0.0");
    let src = server.dir.path().join(format!("src-{DAWN}-1.0.0"));
    sign_source(&src, &wprs_format::Signer::new(&key.secret, None).unwrap()).unwrap();
    wprs_format::build(&src, &wpak, &wprs_format::Limits::default()).unwrap();
    let signed = upload(&server, &ana, &wpak).await;
    assert!(signed["errors"][0].as_str().unwrap().contains("that the repository accepts"), "{signed}");
    let plain = package(server.dir.path(), "org.example.dusk", "1.0.0");
    let sha = wprs_format::sha256_file(&plain).unwrap();
    let block = json!({"sha256": sha, "reason": "known copy"});
    assert_eq!(
        server.json(Method::POST, "/api/v1/mod/blocklist", &admin, Some(block)).await.0,
        StatusCode::CREATED
    );
    let size = fs::metadata(&plain).unwrap().len();
    let (status, _) =
        server.json(Method::POST, "/api/v1/uploads", &ana, Some(json!({"size": size, "sha256": sha}))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        server
            .call(Method::DELETE, &format!("/api/v1/mod/blocklist/{sha}"), &admin)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::NO_CONTENT
    );
    let part = Part::bytes(fs::read(&plain).unwrap()).file_name("dusk.wpak");
    let page = server
        .call(Method::POST, "/upload?back=/account", &ana)
        .multipart(Form::new().part("package", part))
        .send()
        .await
        .unwrap();
    assert_eq!((page.status(), content_type(&page).starts_with("text/html")), (StatusCode::OK, true));
    assert!(page.text().await.unwrap().contains("published"));
    let (_, item) = server.json(Method::GET, "/api/v1/packages/org.example.dusk", &Who::Nobody, None).await;
    assert_eq!(item["owner"], "ana");
    let page = server
        .call(Method::GET, "/packages/org.example.dusk", &Who::Nobody)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(page.contains("Download version 1.0.0"));
}
