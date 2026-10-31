mod common;

use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use common::{Who, start};
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};

fn sealed(fill: u8, blocks: usize) -> String {
    B64.encode([vec![1u8], vec![fill; 24 + 256 * blocks + 16]].concat())
}

fn item(n: usize) -> String {
    format!("{n:064x}")
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
    let ana = server.user(&admin, "ana", "user").await;
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
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        server.call(Method::DELETE, &gone, &ana).send().await.unwrap().status(),
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
