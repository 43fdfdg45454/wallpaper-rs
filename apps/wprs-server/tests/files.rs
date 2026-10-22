mod common;

use base64::Engine;
use base64::prelude::BASE64_STANDARD as B64;
use common::{Who, start};
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

async fn put(server: &common::Server, who: &Who, path: &str, body: Vec<u8>) -> (StatusCode, Value) {
    let answer = server.call(Method::PUT, path, who).body(body).send().await.unwrap();
    (answer.status(), answer.json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn private_files_resume_stay_private_and_respect_the_quota() {
    let server = start(&[("WPRS_FILES_QUOTA_MIB", "1")]).await;
    let admin = server.admin().await;
    for handle in ["ana", "bob"] {
        server.user(&admin, handle, "user").await;
    }
    let ((desk, _), (other, _)) = (server.device("ana").await, server.device("bob").await);
    server.json(Method::POST, "/api/v1/sync/setup", &desk, Some(json!({}))).await;
    let plain = json!({"encrypted": false, "confirm_plaintext": true});
    server.json(Method::POST, "/api/v1/sync/setup", &other, Some(plain)).await;
    let (id, data) = (format!("{:064x}", 7), (0..300_000u32).map(|n| n as u8).collect::<Vec<u8>>());
    let path = format!("/api/v1/sync/blobs/{id}");
    let (status, opened) = server.json(Method::POST, &path, &desk, Some(json!({"size": data.len()}))).await;
    assert_eq!((status, opened["received"].as_i64()), (StatusCode::OK, Some(0)), "{opened}");
    let (status, half) = put(&server, &desk, &format!("{path}?offset=0"), data[..100_000].to_vec()).await;
    assert_eq!((status, half["received"].as_i64()), (StatusCode::OK, Some(100_000)));
    let (status, refused) = put(&server, &desk, &format!("{path}?offset=0"), data[..10].to_vec()).await;
    assert_eq!((status, refused["code"].as_str()), (StatusCode::CONFLICT, Some("offset")));
    assert_eq!(server.json(Method::GET, &path, &desk, None).await.0, StatusCode::CONFLICT);
    let (_, resumed) = server.json(Method::POST, &path, &desk, Some(json!({"size": data.len()}))).await;
    assert_eq!(resumed["received"], 100_000);
    let (status, _) = put(&server, &desk, &format!("{path}?offset=100000"), data[100_000..].to_vec()).await;
    assert_eq!(status, StatusCode::OK);
    let read = |query: &str| server.call(Method::GET, &format!("{path}{query}"), &desk).send();
    assert_eq!(read("").await.unwrap().bytes().await.unwrap(), data);
    assert_eq!(read("?offset=299990&length=50").await.unwrap().bytes().await.unwrap(), data[299_990..]);
    assert_eq!(server.json(Method::GET, &path, &other, None).await.0, StatusCode::NOT_FOUND);
    let (status, _) = server.json(Method::POST, &path, &other, Some(json!({"size": 10}))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let big = format!("/api/v1/sync/blobs/{:064x}", 8);
    let (status, full) = server.json(Method::POST, &big, &desk, Some(json!({"size": 800_000}))).await;
    assert_eq!((status, full["code"].as_str()), (StatusCode::PAYLOAD_TOO_LARGE, Some("quota")));
    let (status, _) = server.json(Method::POST, &path, &desk, Some(json!({"size": 5}))).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, listed) = server.json(Method::GET, "/api/v1/sync/blobs", &desk, None).await;
    assert_eq!(
        (listed["used_bytes"].as_i64(), listed["quota_bytes"].as_i64()),
        (Some(300_000), Some(1 << 20))
    );
    let (_, state) = server.json(Method::GET, "/api/v1/sync/state", &desk, None).await;
    assert_eq!((state["files_bytes"].as_i64(), state["escrow"].as_bool()), (Some(300_000), Some(false)));
    let escrow = json!({"escrow": B64.encode([1u8; 137])});
    let stored = server.call(Method::PUT, "/api/v1/sync/escrow", &desk).json(&escrow).send().await.unwrap();
    assert_eq!(stored.status(), StatusCode::NO_CONTENT);
    assert_eq!(server.json(Method::GET, "/api/v1/sync/state", &desk, None).await.1["escrow"], true);
    let stored = server.dir.path().join("data/private");
    assert_eq!(std::fs::read_dir(&stored).unwrap().count(), 1);
    let (_, export) =
        server.json(Method::GET, "/api/v1/me/export", &server.user(&admin, "cyd", "user").await, None).await;
    assert_eq!(export["files"], json!([]));
    assert_eq!(
        server.call(Method::DELETE, &path, &desk).send().await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        server.call(Method::DELETE, &path, &desk).send().await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    server.json(Method::POST, &path, &desk, Some(json!({"size": 3}))).await;
    put(&server, &desk, &format!("{path}?offset=0"), vec![1, 2, 3]).await;
    let reset = json!({"confirm": "reset"});
    assert_eq!(
        server.json(Method::POST, "/api/v1/sync/reset", &desk, Some(reset)).await.0,
        StatusCode::NO_CONTENT
    );
    assert!(std::fs::read_dir(&stored).unwrap().next().is_none());
}

#[tokio::test]
async fn pieces_need_a_length_lost_bytes_lower_the_offset_and_lists_page() {
    let server = start(&[]).await;
    let admin = server.admin().await;
    server.user(&admin, "ana", "user").await;
    let (desk, _) = server.device("ana").await;
    server.json(Method::POST, "/api/v1/sync/setup", &desk, Some(json!({}))).await;
    let (id, path) = (format!("{:064x}", 9), format!("/api/v1/sync/blobs/{:064x}", 9));
    server.json(Method::POST, &path, &desk, Some(json!({"size": 16}))).await;
    let (Who::Bearer(token), host) = (&desk, server.url.trim_start_matches("http://")) else {
        unreachable!()
    };
    let mut raw = tokio::net::TcpStream::connect(host).await.unwrap();
    let request = format!(
        "PUT {path}?offset=0 HTTP/1.1\r\nHost: {host}\r\nAuthorization: Bearer {token}\r\n\
         Transfer-Encoding: chunked\r\nConnection: close\r\n\r\n4\r\nAAAA\r\n0\r\n\r\n"
    );
    raw.write_all(request.as_bytes()).await.unwrap();
    let mut answer = String::new();
    raw.read_to_string(&mut answer).await.unwrap();
    assert!(answer.starts_with("HTTP/1.1 411"), "{answer}");
    assert_eq!(put(&server, &desk, &format!("{path}?offset=0"), vec![65; 8]).await.0, StatusCode::OK);
    let stored = server.dir.path().join("data/private");
    let file = std::fs::read_dir(&stored).unwrap().next().unwrap().unwrap().path().join(&id);
    std::fs::remove_file(&file).unwrap();
    let (status, lost) = put(&server, &desk, &format!("{path}?offset=8"), vec![66; 8]).await;
    assert_eq!((status, lost["code"].as_str()), (StatusCode::CONFLICT, Some("offset")), "{lost}");
    let (_, reopened) = server.json(Method::POST, &path, &desk, Some(json!({"size": 16}))).await;
    assert_eq!(reopened["received"], 0);
    assert_eq!(put(&server, &desk, &format!("{path}?offset=0"), vec![67; 16]).await.0, StatusCode::OK);
    assert_eq!(std::fs::read(&file).unwrap(), vec![67; 16]);
    let db = rusqlite::Connection::open(server.dir.path().join("data/db/wprs.sqlite")).unwrap();
    let sql = "INSERT INTO sync_blobs (user_id, id, size, received, created_at) \
               SELECT user_id, ?1, 1, 1, 0 FROM sync_blobs LIMIT 1";
    for n in 1..=1004 {
        db.execute(sql, [format!("{:064x}", 100 + n)]).unwrap();
    }
    let (_, first) = server.json(Method::GET, "/api/v1/sync/blobs", &desk, None).await;
    let items = first["items"].as_array().unwrap();
    assert_eq!((items.len(), first["more"].as_bool()), (1000, Some(true)));
    let after = items.last().unwrap()["id"].as_str().unwrap().to_string();
    let (_, rest) = server.json(Method::GET, &format!("/api/v1/sync/blobs?after={after}"), &desk, None).await;
    assert_eq!((rest["items"].as_array().unwrap().len(), rest["more"].as_bool()), (5, Some(false)));
}

#[tokio::test]
async fn the_catalog_searches_with_trigrams_and_falls_back_to_tags() {
    let server = start(&[]).await;
    let db = rusqlite::Connection::open(server.dir.path().join("data/db/wprs.sqlite")).unwrap();
    let add = |id: &str, title: &str, tags: &str, state: &str| {
        db.execute("INSERT INTO packages (id, created_at) VALUES (?1, 1)", [id]).unwrap();
        let sql = "INSERT INTO versions (package_id, version, sha256, size, kind, title, description, license, \
                   tags, authors, state, created_at, published_at) \
                   VALUES (?1, '1.0.0', ?1, 1, 'image', ?2, 'A calm picture', 'CC0-1.0', ?3, '[]', ?4, 1, 1)";
        db.execute(sql, [id, title, tags, state]).unwrap();
    };
    add("org.example.dew", "Morning dew", r#"["sky","garden"]"#, "published");
    add("org.example.glow", "Évening glow", r#"["dusk"]"#, "published");
    add("org.example.harbor", "Harbor lights", r#"["sea"]"#, "pending");
    let found = async |q: &str| {
        let (_, page) = server.json(Method::GET, &format!("/api/v1/catalog?q={q}"), &Who::Nobody, None).await;
        let items = page["items"].as_array().cloned().unwrap_or_default();
        items.iter().map(|i| i["id"].as_str().unwrap().to_string()).collect::<Vec<_>>()
    };
    assert_eq!(found("dew").await, ["org.example.dew"]);
    assert_eq!(found("ORNIN").await, ["org.example.dew"]);
    assert_eq!(found("evening%20glow").await, ["org.example.glow"]);
    assert_eq!(found("calm").await.len(), 2);
    assert_eq!(found("sk").await, ["org.example.dew"]);
    assert_eq!(found("du").await, ["org.example.glow"]);
    assert_eq!(found("%22dew").await, Vec::<String>::new());
    assert!(found("harbor").await.is_empty());
    db.execute("UPDATE versions SET state = 'published' WHERE package_id = 'org.example.harbor'", [])
        .unwrap();
    assert_eq!(found("harbor").await, ["org.example.harbor"]);
    assert_eq!(found("org.example.harb").await, ["org.example.harbor"]);
    db.execute("UPDATE versions SET state = 'removed' WHERE package_id = 'org.example.dew'", []).unwrap();
    assert!(found("dew").await.is_empty());
}
