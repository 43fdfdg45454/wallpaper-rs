mod common;

use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use common::{Who, start};
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use std::time::Duration;

fn sealed(fill: u8) -> String {
    let mut record = vec![1u8];
    record.extend([fill; 24 + 512 + 16]);
    B64.encode(record)
}

fn item(n: u8) -> String {
    format!("{n:02x}").repeat(32)
}

async fn push(server: &common::Server, who: &Who, epoch: i64, changes: Value) -> (StatusCode, Value) {
    server
        .json(Method::POST, "/api/v1/sync/changes", who, Some(json!({"epoch": epoch, "changes": changes})))
        .await
}

#[tokio::test]
async fn encrypted_sync_round_trips_ciphertext_between_devices() {
    let server = start(&[]).await;
    let admin = server.admin().await;
    let ana = server.user(&admin, "ana", "user").await;
    let ((desk, _), (phone, phone_id)) = (server.device("ana").await, server.device("ana").await);
    assert_eq!(server.json(Method::GET, "/api/v1/sync/state", &desk, None).await.0, StatusCode::NOT_FOUND);
    let (status, state) = server.json(Method::POST, "/api/v1/sync/setup", &desk, Some(json!({}))).await;
    assert_eq!(
        (status, state["encrypted"].as_bool(), state["warning"].is_null()),
        (StatusCode::CREATED, Some(true), true)
    );
    assert_eq!(
        server.json(Method::POST, "/api/v1/sync/setup", &phone, Some(json!({}))).await.0,
        StatusCode::CONFLICT
    );
    let fps = B64.encode(br#"{"fps":30}"#);
    let clear = json!([{"collection": "settings", "item": "playback", "base": 0, "payload": fps}]);
    assert_eq!(push(&server, &desk, 0, clear).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let unkeyed =
        json!([{"collection": "library", "item": "org.example.dawn", "base": 0, "payload": sealed(7)}]);
    assert_eq!(push(&server, &desk, 0, unkeyed).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let first = json!([
        {"collection": "library", "item": item(1), "base": 0, "payload": sealed(7)},
        {"collection": "secrets", "item": "provider", "base": 0, "payload": sealed(9)},
    ]);
    let (status, pushed) = push(&server, &desk, 0, first.clone()).await;
    assert_eq!((status, pushed["seq"].as_i64()), (StatusCode::OK, Some(2)), "{pushed}");
    let (_, again) = push(&server, &desk, 0, first).await;
    assert_eq!((again["seq"].as_i64(), again["results"][0]["status"].as_str()), (Some(2), Some("ok")));
    let (_, pulled) = server.json(Method::GET, "/api/v1/sync/changes?since=0", &phone, None).await;
    assert_eq!(pulled["changes"].as_array().unwrap().len(), 2);
    assert_eq!(pulled["changes"][0]["payload"], sealed(7));
    let race = json!([{"collection": "library", "item": item(1), "base": 0, "payload": sealed(8)}]);
    let (_, raced) = push(&server, &phone, 0, race).await;
    assert_eq!(
        (raced["results"][0]["status"].as_str(), raced["results"][0]["current"]["seq"].as_i64()),
        (Some("conflict"), Some(1))
    );
    let merged = json!([{"collection": "library", "item": item(1), "base": 1, "payload": sealed(8)}]);
    assert_eq!(push(&server, &phone, 0, merged).await.1["seq"], 3);
    let tomb = json!([{"collection": "library", "item": item(1), "base": 3, "deleted": true}]);
    assert_eq!(push(&server, &desk, 0, tomb).await.1["seq"], 4);
    let (_, pulled) = server.json(Method::GET, "/api/v1/sync/changes?since=2", &phone, None).await;
    assert_eq!(
        (pulled["changes"][0]["deleted"].as_bool(), pulled["changes"][0]["payload"].is_null()),
        (Some(true), true)
    );
    let (status, ahead) = server.json(Method::GET, "/api/v1/sync/changes?since=9", &phone, None).await;
    assert_eq!((status, ahead["code"].as_str()), (StatusCode::GONE, Some("resync")));
    let (status, rotated) =
        server.json(Method::POST, "/api/v1/sync/epoch", &desk, Some(json!({"epoch": 1}))).await;
    assert_eq!((status, rotated["epoch"].as_i64()), (StatusCode::OK, Some(1)));
    let stale = json!([{"collection": "playlists", "item": "evening", "base": 0, "payload": sealed(3)}]);
    assert_eq!(push(&server, &desk, 0, stale).await.1["code"], "epoch_mismatch");
    let escrow = json!({"escrow": B64.encode([5u8; 80])});
    assert_eq!(
        server.call(Method::PUT, "/api/v1/sync/escrow", &desk).json(&escrow).send().await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(server.json(Method::GET, "/api/v1/sync/escrow", &phone, None).await.1, escrow);
    let (_, export) = server.json(Method::GET, "/api/v1/me/export", &ana, None).await;
    let stored: Vec<&Value> = export["records"].as_array().unwrap().iter().map(|r| &r["payload"]).collect();
    assert_eq!(stored, [&Value::from(sealed(9)), &Value::Null]);
    let message = json!({"to_device": phone_id, "kind": "key-share", "body": B64.encode([4u8; 120])});
    let (status, sent) = server.json(Method::POST, "/api/v1/mailbox", &desk, Some(message)).await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, inbox) = server.json(Method::GET, "/api/v1/mailbox", &phone, None).await;
    assert_eq!(
        (inbox["items"][0]["kind"].as_str(), inbox["items"][0]["body"].as_str()),
        (Some("key-share"), Some(B64.encode([4u8; 120]).as_str()))
    );
    assert!(
        server.json(Method::GET, "/api/v1/mailbox", &desk, None).await.1["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let ack = format!("/api/v1/mailbox/{}", sent["id"]);
    assert_eq!(
        server.call(Method::DELETE, &ack, &phone).send().await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    assert!(
        server.json(Method::GET, "/api/v1/mailbox", &phone, None).await.1["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn plaintext_needs_confirmation_and_events_stream() {
    let server = start(&[]).await;
    let admin = server.admin().await;
    server.user(&admin, "ana", "user").await;
    let ((desk, _), (phone, _)) = (server.device("ana").await, server.device("ana").await);
    let (status, refused) =
        server.json(Method::POST, "/api/v1/sync/setup", &desk, Some(json!({"encrypted": false}))).await;
    assert_eq!(
        (status, refused["detail"].as_str().unwrap().contains("confirm_plaintext")),
        (StatusCode::UNPROCESSABLE_ENTITY, true)
    );
    let (_, state) = server
        .json(
            Method::POST,
            "/api/v1/sync/setup",
            &desk,
            Some(json!({"encrypted": false, "confirm_plaintext": true})),
        )
        .await;
    assert_eq!((state["encrypted"].as_bool(), state["warning"].is_string()), (Some(false), true));
    let secret =
        json!([{"collection": "secrets", "item": "provider", "base": 0, "payload": B64.encode(b"key")}]);
    assert_eq!(push(&server, &desk, 0, secret).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let mut events = server.call(Method::GET, "/api/v1/sync/events", &phone).send().await.unwrap();
    assert_eq!(events.status(), StatusCode::OK);
    assert_eq!(
        server.call(Method::GET, "/api/v1/sync/events", &phone).send().await.unwrap().status(),
        StatusCode::CONFLICT
    );
    let fps = B64.encode(br#"{"fps":30}"#);
    let settings = json!([{"collection": "settings", "item": "playback", "base": 0, "payload": fps}]);
    assert_eq!(push(&server, &desk, 0, settings).await.1["seq"], 1);
    let chunk = tokio::time::timeout(Duration::from_secs(5), events.chunk()).await.unwrap().unwrap().unwrap();
    let text = String::from_utf8_lossy(&chunk);
    assert!(text.contains("event: seq") && text.contains(r#"{"seq":1}"#), "{text}");
    drop(events);
    let mut reset =
        server.json(Method::POST, "/api/v1/sync/reset", &desk, Some(json!({"confirm": "reset"}))).await;
    assert_eq!(reset.0, StatusCode::NO_CONTENT);
    reset = server.json(Method::GET, "/api/v1/sync/state", &desk, None).await;
    assert_eq!(reset.0, StatusCode::NOT_FOUND);
}
