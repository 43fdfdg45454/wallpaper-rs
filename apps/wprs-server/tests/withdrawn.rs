mod common;

use common::{Who, content_type, package, start, upload};
use reqwest::multipart::{Form, Part};
use reqwest::{Method, StatusCode};
use serde_json::json;
use std::fs;

#[tokio::test(flavor = "multi_thread")]
async fn moderators_never_judge_their_own_uploads_and_refused_files_leave_the_disk() {
    let server = start(&[("WPRS_AUTO_PUBLISH", "never"), ("WPRS_UPLOADS_PER_DAY", "3")]).await;
    wprs_server::init(&server.cfg, "Example wallpapers", false, false).unwrap();
    let admin = server.admin().await;
    let mia = server.user(&admin, "mia", "moderator").await;
    let status = upload(&server, &mia, &package(server.dir.path(), "org.example.dawn", "1.0.0")).await;
    assert_eq!(status["state"], "pending_review");
    let (_, queue) = server.json(Method::GET, "/api/v1/mod/queue", &mia, None).await;
    let (case, sha) = (
        queue["items"][0]["id"].as_i64().unwrap(),
        queue["items"][0]["sha256"].as_str().unwrap().to_string(),
    );
    let path = format!("/api/v1/mod/cases/{case}/decision");
    let approve = json!({"decision": "approve"});
    assert_eq!(server.json(Method::POST, &path, &mia, Some(approve)).await.0, StatusCode::FORBIDDEN);
    let (blob, thumb) = (
        server.cfg.data.join(format!("blobs/{sha}.wpak")),
        server.cfg.data.join(format!("thumbs/{sha}.jpg")),
    );
    assert!(blob.exists() && thumb.exists());
    let reject = json!({"decision": "reject", "statement": "The preview shows another work."});
    assert_eq!(server.json(Method::POST, &path, &admin, Some(reject)).await.0, StatusCode::OK);
    assert!(!blob.exists() && !thumb.exists());
    let dusk = fs::read(package(server.dir.path(), "org.example.dusk", "1.0.0")).unwrap();
    let noisy = Form::new()
        .text("note", "x".repeat(10_000))
        .part("package", Part::bytes(dusk.clone()).file_name("dusk.wpak"));
    let refused = server.call(Method::POST, "/upload", &mia).multipart(noisy).send().await.unwrap();
    assert_eq!(refused.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let form = || Form::new().part("package", Part::bytes(dusk.clone()).file_name("dusk.wpak"));
    let sent = server.call(Method::POST, "/upload", &mia).multipart(form()).send().await.unwrap();
    assert_eq!((sent.status(), content_type(&sent).starts_with("text/html")), (StatusCode::OK, true));
    let late = server.call(Method::POST, "/upload", &mia).multipart(form()).send().await.unwrap();
    assert_eq!(late.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test(flavor = "multi_thread")]
async fn withdrawn_packages_stay_held_and_a_counter_notice_restores_them() {
    let server = start(&[]).await;
    wprs_server::init(&server.cfg, "Example wallpapers", false, false).unwrap();
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "trusted").await;
    let own = upload(&server, &admin, &package(server.dir.path(), "org.example.dusk", "1.0.0")).await;
    let shared = upload(&server, &ana, &package(server.dir.path(), "org.example.dawn", "1.0.0")).await;
    assert_eq!((own["state"].as_str(), shared["state"].as_str()), (Some("published"), Some("published")));
    let notice = |kind: &'static str, id: &'static str| {
        let location = format!("{}/packages/{id}", server.url);
        let fields = [
            ("kind", kind.to_string()),
            ("location", location),
            ("explanation", "This picture copies a protected work.".into()),
            ("name", "Rights Holder".into()),
            ("email", "holder@example.org".into()),
            ("statements", "true".into()),
            ("signature", "Rights Holder".into()),
        ];
        server
            .call(
                Method::POST,
                if kind == "counter" { "/legal/counter-notice" } else { "/legal/notice" },
                &Who::Nobody,
            )
            .form(&fields)
            .send()
    };
    let case = async |package: &str| {
        let (_, queue) = server.json(Method::GET, "/api/v1/mod/queue", &admin, None).await;
        let items = queue["items"].as_array().unwrap().clone();
        items.iter().find(|c| c["package"] == package && c["reason"] != "upload").unwrap()["id"]
            .as_i64()
            .unwrap()
    };
    let decide = async |case: i64, decision: &str| {
        let body = json!({"decision": decision, "statement": "Removed after a legal notice."});
        server.json(Method::POST, &format!("/api/v1/mod/cases/{case}/decision"), &admin, Some(body)).await
    };
    assert_eq!(notice("illegal", "org.example.dusk").await.unwrap().status(), StatusCode::OK);
    let mine = case("org.example.dusk").await;
    assert_eq!(decide(mine, "dismiss").await.0, StatusCode::FORBIDDEN);
    assert_eq!(decide(mine, "remove").await.0, StatusCode::OK);
    assert_eq!(notice("copyright", "org.example.dawn").await.unwrap().status(), StatusCode::OK);
    assert_eq!(decide(case("org.example.dawn").await, "remove").await.0, StatusCode::OK);
    let data = server.dir.path().join("data");
    let public = data.join("repo/packages/org.example.dawn/1.0.0.wpak");
    let held: Vec<String> = fs::read_dir(data.join("held"))
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert!(!public.exists() && held.iter().filter(|n| n.ends_with(".wpak")).count() == 2, "{held:?}");
    let backup = wprs_server::backup(&server.cfg, None).unwrap();
    let manifest = fs::read_to_string(backup.join("manifest.json")).unwrap();
    assert!(manifest.contains("\"held/"));
    assert_eq!(notice("counter", "org.example.dawn").await.unwrap().status(), StatusCode::OK);
    let (status, restored) = decide(case("org.example.dawn").await, "restore").await;
    assert_eq!((status, restored["state"].as_str()), (StatusCode::OK, Some("published")), "{restored}");
    assert!(public.is_file());
    let (_, item) = server.json(Method::GET, "/api/v1/packages/org.example.dawn", &Who::Nobody, None).await;
    assert_eq!(item["version"], "1.0.0");
    for entry in fs::read_dir(data.join("held")).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    assert_eq!(notice("counter", "org.example.dusk").await.unwrap().status(), StatusCode::OK);
    let counter = case("org.example.dusk").await;
    assert_eq!(decide(counter, "restore").await.0, StatusCode::FORBIDDEN);
    let mia = server.user(&admin, "mia", "moderator").await;
    let path = format!("/api/v1/mod/cases/{counter}/decision");
    let (status, gone) = server.json(Method::POST, &path, &mia, Some(json!({"decision": "restore"}))).await;
    assert_eq!((status, gone["code"].as_str()), (StatusCode::CONFLICT, Some("deleted")));
}
