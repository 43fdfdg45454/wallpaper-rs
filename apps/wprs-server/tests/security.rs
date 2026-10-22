mod common;

use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use common::{Who, content_type, cookie, package, start, upload};
use reqwest::multipart::{Form, Part};
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use std::fs;

fn sealed(fill: u8, blocks: usize) -> String {
    B64.encode([vec![1u8], vec![fill; 24 + 256 * blocks + 16]].concat())
}

fn item(n: usize) -> String {
    format!("{n:064x}")
}

fn sql(server: &common::Server, statement: &str) {
    let db = rusqlite::Connection::open(server.dir.path().join("data/db/wprs.sqlite")).unwrap();
    db.execute_batch(statement).unwrap();
}

#[tokio::test]
async fn passwords_change_only_from_the_web_and_keep_devices_unless_asked() {
    let server = start(&[]).await;
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "user").await;
    let ((desk, _), (phone, _)) = (server.device("ana").await, server.device("ana").await);
    let (_, state) = server.json(Method::POST, "/api/v1/sync/setup", &desk, Some(json!({}))).await;
    assert_eq!(state["rotate"], false);
    let (_, token) = server
        .json(Method::POST, "/api/v1/me/tokens", &ana, Some(json!({"name": "script", "scopes": "upload"})))
        .await;
    let script = Who::Bearer(token["token"].as_str().unwrap().into());
    let change = json!({"current": "a long enough password", "new": "a brand new password"});
    for who in [&script, &desk] {
        let (status, _) = server.json(Method::POST, "/api/v1/me/password", who, Some(change.clone())).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let start = json!({"platform": "linux"});
    let (_, started) =
        server.json(Method::POST, "/api/v1/auth/device/start", &Who::Nobody, Some(start)).await;
    let approve = json!({"user_code": started["user_code"]});
    assert_eq!(
        server.json(Method::POST, "/api/v1/auth/device/approve", &desk, Some(approve)).await.0,
        StatusCode::OK
    );
    let fields = [("current", "a long enough password"), ("new", "a brand new password")];
    let answer = server.form("/api/v1/me/password", &ana, &fields).await;
    assert_eq!(answer.status(), StatusCode::OK);
    let web = Who::Cookie(cookie(&answer));
    let poll = json!({"device_code": started["device_code"]});
    let (_, refused) = server.json(Method::POST, "/api/v1/auth/device/token", &Who::Nobody, Some(poll)).await;
    assert_eq!(refused["code"], "invalid_grant");
    for gone in [&ana, &script] {
        assert_eq!(server.json(Method::GET, "/api/v1/me", gone, None).await.0, StatusCode::UNAUTHORIZED);
    }
    for kept in [&web, &desk, &phone] {
        assert_eq!(server.json(Method::GET, "/api/v1/me", kept, None).await.0, StatusCode::OK);
    }
    assert_eq!(server.json(Method::GET, "/api/v1/sync/state", &desk, None).await.1["rotate"], false);
    let fields =
        [("current", "a brand new password"), ("new", "the third new password"), ("devices", "true")];
    let answer = server.form("/api/v1/me/password", &web, &fields).await;
    assert_eq!(answer.status(), StatusCode::OK);
    let renewed = Who::Cookie(cookie(&answer));
    for gone in [&web, &desk, &phone] {
        assert_eq!(server.json(Method::GET, "/api/v1/me", gone, None).await.0, StatusCode::UNAUTHORIZED);
    }
    let (_, devices) = server.json(Method::GET, "/api/v1/me/devices", &renewed, None).await;
    assert_eq!(devices["items"].as_array().unwrap().len(), 0);
    let db = rusqlite::Connection::open(server.dir.path().join("data/db/wprs.sqlite")).unwrap();
    assert_eq!(db.query_row("SELECT rotate FROM sync_heads", [], |r| r.get::<_, i64>(0)).unwrap(), 1);
}

#[tokio::test]
async fn an_account_without_a_password_needs_a_fresh_web_sign_in_to_set_one() {
    let server = start(&[]).await;
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "user").await;
    let (desk, _) = server.device("ana").await;
    sql(&server, "UPDATE users SET password_hash = NULL WHERE handle = 'ana'");
    let change = json!({"current": "", "new": "attacker chosen password"});
    let (status, _) = server.json(Method::POST, "/api/v1/me/password", &desk, Some(change.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    sql(&server, "UPDATE sessions SET created_at = created_at - 3600000");
    let (status, refused) =
        server.json(Method::POST, "/api/v1/me/password", &ana, Some(change.clone())).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{refused}");
    sql(&server, "UPDATE sessions SET created_at = created_at + 3600000");
    let (status, _) = server.json(Method::POST, "/api/v1/me/password", &ana, Some(change)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn password_guesses_from_a_signed_in_session_are_limited() {
    let server = start(&[("WPRS_RATE_LIMITS", "true")]).await;
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "user").await;
    let wrong = json!({"current": "not the password at all", "new": "a brand new password"});
    let mut statuses = Vec::new();
    for _ in 0..8 {
        statuses.push(server.json(Method::POST, "/api/v1/me/password", &ana, Some(wrong.clone())).await.0);
    }
    assert!(
        statuses.contains(&StatusCode::FORBIDDEN) && statuses.ends_with(&[StatusCode::TOO_MANY_REQUESTS])
    );
    let right = json!({"current": "a long enough password", "new": "a brand new password"});
    let (status, _) = server.json(Method::POST, "/api/v1/me/password", &ana, Some(right)).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn pending_codes_show_what_the_device_sent_and_nothing_else() {
    let server = start(&[]).await;
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "user").await;
    let (desk, _) = server.device("ana").await;
    let (_, devices) = server.json(Method::GET, "/api/v1/me/tokens", &desk, None).await;
    assert_eq!(devices["items"][0]["scopes"], "upload sync devices");
    let start = json!({"platform": "android", "client": "wallpaper-rs/9.9"});
    let (_, started) =
        server.json(Method::POST, "/api/v1/auth/device/start", &Who::Nobody, Some(start)).await;
    let code = json!({"user_code": started["user_code"]});
    let (status, shown) =
        server.json(Method::POST, "/api/v1/auth/device/preview", &desk, Some(code.clone())).await;
    assert_eq!(
        (status, shown["platform"].as_str(), shown["client"].as_str()),
        (StatusCode::OK, Some("android"), Some("wallpaper-rs/9.9"))
    );
    let fields: Vec<&String> = shown.as_object().unwrap().keys().collect();
    assert_eq!(fields, ["client", "expires_at", "platform", "requested_at"]);
    assert!(!shown.to_string().contains("127.0.0.1"));
    let page =
        server.call(Method::GET, &format!("/link?code={}", started["user_code"].as_str().unwrap()), &ana);
    let page = page.send().await.unwrap().text().await.unwrap();
    assert!(page.contains("wallpaper-rs/9.9") && page.contains("your own new device"));
    let unknown = json!({"user_code": "BBBB-BBBB"});
    assert_eq!(
        server.json(Method::POST, "/api/v1/auth/device/preview", &desk, Some(unknown)).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, token) = server
        .json(Method::POST, "/api/v1/me/tokens", &ana, Some(json!({"name": "script", "scopes": "sync"})))
        .await;
    let script = Who::Bearer(token["token"].as_str().unwrap().into());
    assert_eq!(
        server.json(Method::POST, "/api/v1/auth/device/preview", &script, Some(code)).await.0,
        StatusCode::FORBIDDEN
    );
    let long = json!({"platform": "linux", "client": "x".repeat(65)});
    assert_eq!(
        server.json(Method::POST, "/api/v1/auth/device/start", &Who::Nobody, Some(long)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}

async fn push(server: &common::Server, who: &Who, epoch: i64, changes: Value) -> (StatusCode, Value) {
    server
        .json(Method::POST, "/api/v1/sync/changes", who, Some(json!({"epoch": epoch, "changes": changes})))
        .await
}

#[tokio::test]
async fn rotations_finish_by_dropping_old_records_and_pages_stay_small() {
    let server = start(&[]).await;
    let admin = server.admin().await;
    server.user(&admin, "ana", "user").await;
    let ((desk, _), (phone, phone_id)) = (server.device("ana").await, server.device("ana").await);
    let first = json!({"key_id": "0".repeat(32)});
    let (_, state) = server.json(Method::POST, "/api/v1/sync/setup", &desk, Some(first)).await;
    assert_eq!((state["epoch"].as_i64(), state["key_id"].as_str()), (Some(0), Some("0".repeat(32).as_str())));
    for batch in 0..4 {
        let changes: Vec<Value> = (0..5)
            .map(|n| json!({"collection": "library", "item": item(batch * 5 + n), "base": 0, "payload": sealed(7, 1000)}))
            .collect();
        assert_eq!(push(&server, &desk, 0, Value::from(changes)).await.0, StatusCode::OK);
    }
    let (_, page) = server.json(Method::GET, "/api/v1/sync/changes?since=0", &phone, None).await;
    let count = page["changes"].as_array().unwrap().len();
    assert!((1..20).contains(&count) && page["more"] == true, "{count}");
    assert_eq!(page["changes"][0]["epoch"], 0);
    let gone = format!("/api/v1/me/devices/{phone_id}");
    assert_eq!(
        server.call(Method::DELETE, &gone, &desk).send().await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(server.json(Method::GET, "/api/v1/sync/state", &desk, None).await.1["rotate"], true);
    let bad = json!({"epoch": 2, "key_id": "1".repeat(32)});
    assert_eq!(
        server.json(Method::POST, "/api/v1/sync/epoch", &desk, Some(bad)).await.1["code"],
        "epoch_mismatch"
    );
    let next = json!({"epoch": 1, "key_id": "1".repeat(32)});
    let (_, state) = server.json(Method::POST, "/api/v1/sync/epoch", &desk, Some(next.clone())).await;
    assert_eq!((state["epoch"].as_i64(), state["rotate"].as_bool()), (Some(1), Some(false)));
    assert_eq!(server.json(Method::POST, "/api/v1/sync/epoch", &desk, Some(next)).await.0, StatusCode::OK);
    let old = json!([{"collection": "library", "item": item(30), "base": 0, "payload": sealed(1, 1)}]);
    assert_eq!(push(&server, &desk, 0, old).await.1["code"], "epoch_mismatch");
    let rewrite = json!([{"collection": "library", "item": item(0), "base": 1, "payload": sealed(2, 1)}]);
    assert_eq!(push(&server, &desk, 1, rewrite).await.0, StatusCode::OK);
    let done = json!({"epoch": 1, "done": true});
    let (_, state) = server.json(Method::POST, "/api/v1/sync/epoch", &desk, Some(done)).await;
    assert_eq!(state["used_bytes"].as_i64(), Some(41 + 256));
    let (_, page) = server.json(Method::GET, "/api/v1/sync/changes?since=0", &desk, None).await;
    assert_eq!(
        (page["changes"].as_array().unwrap().len(), page["changes"][0]["epoch"].as_i64()),
        (1, Some(1))
    );
    let (mail, mail_id) = server.device("ana").await;
    for _ in 0..20 {
        let message = json!({"to_device": mail_id, "kind": "key-share", "body": B64.encode([4u8; 60_000])});
        assert_eq!(
            server.json(Method::POST, "/api/v1/mailbox", &desk, Some(message)).await.0,
            StatusCode::CREATED
        );
    }
    let (_, inbox) = server.json(Method::GET, "/api/v1/mailbox", &mail, None).await;
    let count = inbox["items"].as_array().unwrap().len();
    assert!((1..20).contains(&count) && inbox["more"] == true, "{count}");
}

#[tokio::test(flavor = "multi_thread")]
async fn moderators_never_judge_their_own_uploads_and_refused_files_leave_the_disk() {
    let server = start(&[("WPRS_AUTO_PUBLISH", "never"), ("WPRS_UPLOADS_PER_DAY", "3")]).await;
    wprs_server::init(&server.cfg, "Example wallpapers", false).unwrap();
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
    wprs_server::init(&server.cfg, "Example wallpapers", false).unwrap();
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
