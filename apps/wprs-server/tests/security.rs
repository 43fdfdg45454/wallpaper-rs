mod common;

use common::{Who, cookie, start};
use reqwest::{Method, StatusCode};
use serde_json::json;

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
    let (status, refused) =
        server.json(Method::POST, "/api/v1/auth/device/approve", &desk, Some(approve)).await;
    assert_eq!(
        (status, refused["detail"].as_str().unwrap().contains("password")),
        (StatusCode::FORBIDDEN, true)
    );
    let wrong = json!({"user_code": started["user_code"], "password": "not the password"});
    let (status, _) = server.json(Method::POST, "/api/v1/auth/device/approve", &desk, Some(wrong)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = server
        .json(
            Method::POST,
            "/api/v1/auth/device/preview",
            &desk,
            Some(json!({"user_code": started["user_code"]})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let approve = json!({"user_code": started["user_code"], "password": "a long enough password"});
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
    assert_eq!(devices["items"][0]["scopes"], "upload sync");
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
        server.json(Method::POST, "/api/v1/auth/device/preview", &script, Some(code.clone())).await.0,
        StatusCode::FORBIDDEN
    );
    let approve = json!({"user_code": started["user_code"], "approve": true});
    let (status, _) = server.json(Method::POST, "/api/v1/auth/device/approve", &script, Some(approve)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let wide = json!({"name": "devices", "scopes": "sync devices"});
    let (status, _) = server.json(Method::POST, "/api/v1/me/tokens", &ana, Some(wide)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let long = json!({"platform": "linux", "client": "x".repeat(65)});
    assert_eq!(
        server.json(Method::POST, "/api/v1/auth/device/start", &Who::Nobody, Some(long)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}
