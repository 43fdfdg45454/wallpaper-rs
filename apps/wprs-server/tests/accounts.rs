mod common;

use common::{Who, content_type, cookie, start};
use reqwest::{Method, StatusCode};
use serde_json::json;

#[tokio::test]
async fn first_admin_registration_and_tokens() {
    let server = start(&[]).await;
    let page = server.call(Method::GET, "/setup", &Who::Nobody).send().await.unwrap();
    assert!(page.headers()["content-security-policy"].to_str().unwrap().contains("default-src 'none'"));
    let wrong = server
        .form(
            "/setup",
            &Who::Nobody,
            &[("token", "wprs_s_bad"), ("handle", "root"), ("password", "correct horse battery")],
        )
        .await;
    assert_eq!(wrong.status(), StatusCode::FORBIDDEN);
    assert!(content_type(&wrong).starts_with("text/html"));
    let admin = server.admin().await;
    let account = server.call(Method::GET, "/account", &admin).send().await.unwrap();
    assert_eq!(account.headers()["cache-control"], "no-store");
    assert!(account.text().await.unwrap().contains("Signed in as"));
    let open = json!({"handle": "eve", "password": "a long enough password"});
    assert_eq!(
        server.json(Method::POST, "/api/v1/auth/register", &Who::Nobody, Some(open)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let user = server.user(&admin, "ana", "user").await;
    for next in ["/\t/example.org", "//example.org", "/\\example.org", "https://example.org"] {
        let fields = [("handle", "ana"), ("password", "a long enough password"), ("next", next)];
        assert_eq!(server.form("/login", &Who::Nobody, &fields).await.headers()["location"], "/account");
    }
    let reuse = json!({"handle": "bob", "password": "a long enough password", "invite": "0000"});
    assert_eq!(
        server.json(Method::POST, "/api/v1/auth/register", &Who::Nobody, Some(reuse)).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, me) = server.json(Method::GET, "/api/v1/me", &user, None).await;
    assert_eq!(
        (status, me["handle"].as_str(), me["role"].as_str()),
        (StatusCode::OK, Some("ana"), Some("user"))
    );
    let staff = json!({"name": "ci", "scopes": "mod"});
    assert_eq!(
        server.json(Method::POST, "/api/v1/me/tokens", &user, Some(staff)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, token) = server
        .json(Method::POST, "/api/v1/me/tokens", &user, Some(json!({"name": "ci", "scopes": "upload"})))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let bearer = Who::Bearer(token["token"].as_str().unwrap().into());
    assert_eq!(server.json(Method::GET, "/api/v1/me", &bearer, None).await.1["handle"], "ana");
    assert_eq!(
        server
            .json(Method::POST, "/api/v1/me/tokens", &bearer, Some(json!({"name": "x", "scopes": "upload"})))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(server.json(Method::GET, "/api/v1/sync/state", &bearer, None).await.0, StatusCode::FORBIDDEN);
    let real = token["token"].as_str().unwrap();
    let forged = Who::Bearer(format!("{}{}", &real[..79], if real.ends_with('0') { 1 } else { 0 }));
    assert_eq!(server.json(Method::GET, "/api/v1/me", &forged, None).await.0, StatusCode::UNAUTHORIZED);
    let revoke = format!("/api/v1/me/tokens/{}?_method=DELETE&back=/account", token["id"]);
    let done = server.form(&revoke, &user, &[]).await;
    assert_eq!((done.status(), content_type(&done).starts_with("text/html")), (StatusCode::OK, true));
    assert_eq!(server.json(Method::GET, "/api/v1/me", &bearer, None).await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn devices_get_generic_names_and_enroll_with_codes() {
    let server = start(&[]).await;
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "user").await;
    let wrong = json!({"handle": "ana", "password": "not the password", "device": {"platform": "linux"}});
    assert_eq!(
        server.json(Method::POST, "/api/v1/auth/login", &Who::Nobody, Some(wrong)).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (first, _) = server.device("ana").await;
    let (status, list) = server.json(Method::GET, "/api/v1/me/devices", &first, None).await;
    assert_eq!((status, list["items"][0]["name"].as_str()), (StatusCode::OK, Some("Device 1")));
    let (_, pending) = server
        .json(Method::POST, "/api/v1/auth/device/start", &Who::Nobody, Some(json!({"platform": "android"})))
        .await;
    let poll = json!({"device_code": pending["device_code"]});
    let (status, body) =
        server.json(Method::POST, "/api/v1/auth/device/token", &Who::Nobody, Some(poll.clone())).await;
    assert_eq!((status, body["code"].as_str()), (StatusCode::BAD_REQUEST, Some("authorization_pending")));
    let (_, body) = server.json(Method::POST, "/api/v1/auth/device/token", &Who::Nobody, Some(poll)).await;
    assert_eq!(body["code"], "slow_down");
    let key = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
    let (_, started) = server
        .json(
            Method::POST,
            "/api/v1/auth/device/start",
            &Who::Nobody,
            Some(json!({"platform": "android", "enc_key": key})),
        )
        .await;
    assert!(
        started["verification_uri_complete"]
            .as_str()
            .unwrap()
            .ends_with(started["user_code"].as_str().unwrap())
    );
    let link = server
        .call(Method::GET, &format!("/link?code={}", started["user_code"].as_str().unwrap()), &ana)
        .send()
        .await
        .unwrap();
    assert!(link.text().await.unwrap().contains(started["user_code"].as_str().unwrap()));
    let code = started["user_code"].as_str().unwrap().to_lowercase().replace('-', "");
    let approved = server
        .form("/api/v1/auth/device/approve?back=/account", &ana, &[("user_code", &code), ("approve", "true")])
        .await;
    assert_eq!(approved.status(), StatusCode::OK);
    let (status, enrolled) = server
        .json(
            Method::POST,
            "/api/v1/auth/device/token",
            &Who::Nobody,
            Some(json!({"device_code": started["device_code"]})),
        )
        .await;
    assert_eq!((status, enrolled["device_name"].as_str()), (StatusCode::OK, Some("Device 2")));
    let phone = Who::Bearer(enrolled["token"].as_str().unwrap().into());
    let (_, list) = server.json(Method::GET, "/api/v1/me/devices", &phone, None).await;
    assert_eq!(list["items"][1]["enc_key"], key);
    let gone = format!("/api/v1/me/devices/{}", enrolled["device_id"].as_str().unwrap());
    assert_eq!(
        server.call(Method::DELETE, &gone, &first).send().await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let (_, mine) = server.json(Method::GET, "/api/v1/me/tokens", &ana, None).await;
    let devices = mine["items"].as_array().unwrap().iter().filter(|t| !t["device_id"].is_null());
    assert!(devices.clone().count() == 2 && devices.clone().all(|t| t["scopes"] == "upload sync"), "{mine}");
    assert_eq!(
        server.call(Method::DELETE, &gone, &ana).send().await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(server.json(Method::GET, "/api/v1/me", &phone, None).await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn csrf_rate_limits_and_password_changes() {
    let server = start(&[("WPRS_RATE_LIMITS", "true")]).await;
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "user").await;
    let cross = server
        .call(Method::POST, "/api/v1/me/password", &ana)
        .header("origin", "https://example.org")
        .form(&[("current", "x"), ("new", "y")]);
    assert_eq!(cross.send().await.unwrap().status(), StatusCode::FORBIDDEN);
    let fetch = server
        .call(Method::POST, "/logout", &ana)
        .header("sec-fetch-site", "cross-site")
        .send()
        .await
        .unwrap();
    assert_eq!(fetch.status(), StatusCode::FORBIDDEN);
    let change = [("current", "a long enough password"), ("new", "another long password")];
    assert_eq!(server.form("/api/v1/me/password", &ana, &change).await.status(), StatusCode::OK);
    let mut statuses = Vec::new();
    for _ in 0..6 {
        let response = server
            .form("/login", &Who::Nobody, &[("handle", "ana"), ("password", "another long password")])
            .await;
        if response.status() == StatusCode::SEE_OTHER {
            assert!(!cookie(&response).is_empty());
        }
        statuses.push((response.status(), response.headers().get("retry-after").is_some()));
    }
    assert_eq!(statuses.iter().filter(|(s, _)| *s == StatusCode::SEE_OTHER).count(), 1);
    assert_eq!(statuses.last(), Some(&(StatusCode::TOO_MANY_REQUESTS, true)));
}
