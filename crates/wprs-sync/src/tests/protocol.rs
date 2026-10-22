use super::{at, doc};
use crate::*;
use serde_json::json;
use std::time::Duration;

fn server(code: &str) -> Result<serde_json::Value> {
    Err(Error::Server { status: 400, code: code.into(), detail: String::new() })
}

#[test]
fn enrollment_polls_slower_when_asked_and_stops_on_refusal() {
    let mut interval = 5;
    assert!(
        matches!(poll(&mut interval, server("authorization_pending")), Poll::Wait(d) if d == Duration::from_secs(5))
    );
    assert!(
        matches!(poll(&mut interval, server("slow_down")), Poll::Wait(d) if d == Duration::from_secs(10))
    );
    assert!(matches!(poll(&mut interval, Err(Error::Network("down".into()))), Poll::Wait(_)));
    assert!(
        matches!(poll(&mut interval, server("access_denied")), Poll::Stop(e) if e.code() == "access_denied")
    );
    assert!(matches!(poll(&mut interval, server("expired_token")), Poll::Stop(_)));
    assert!(matches!(poll(&mut interval, Ok(json!({"token": "t"}))), Poll::Done(_)));
    assert_eq!(interval, 10);
}

#[test]
fn server_sent_events_survive_split_chunks() {
    let mut parser = Parser::default();
    assert!(parser.feed(b": keep-alive\n\nevent: se").unwrap().is_empty());
    let events = parser.feed(b"q\ndata: {\"seq\": 4}\r\n\r\nevent: mail\ndata: {\"id\": 9}\n\n").unwrap();
    assert_eq!(events, [("seq".to_string(), json!({"seq": 4})), ("mail".to_string(), json!({"id": 9}))]);
}

fn session(encrypted: bool, key: SyncKey) -> Session {
    Session::new(Api::new("http://127.0.0.1:9").unwrap(), 3, 1, encrypted, key)
}

#[test]
fn sessions_seal_records_only_for_their_account_and_place() {
    let key = SyncKey::generate();
    let copy = SyncKey::from_bytes(&key.bytes()).unwrap();
    let (sealed, plain) = (session(true, key), session(false, SyncKey::generate()));
    let record = doc(&[("t:secret-tag", json!(true), 5)]);
    let payload = sealed.encode("library/ab", &record).unwrap();
    assert!(!payload.contains("secret"));
    assert_eq!(sealed.decode("library/ab", &payload, 1).unwrap(), record);
    assert!(sealed.decode("library/cd", &payload, 1).is_err());
    assert!(sealed.decode("library/ab", &payload, 0).is_err());
    let mut other = session(true, copy);
    other.user = 4;
    assert!(other.decode("library/ab", &payload, 1).is_err());
    other.user = 3;
    let mut next = session(true, other.key.rotated());
    next.epoch = 2;
    let fresh = next.encode("library/ab", &record).unwrap();
    next.old = Some(other.key);
    assert_eq!(next.decode("library/ab", &payload, 1).unwrap(), record);
    assert_eq!(next.decode("library/ab", &fresh, 2).unwrap(), record);
    assert!(next.decode("library/ab", &fresh, 1).is_err());
    let open = plain.encode("library/ab", &record).unwrap();
    assert_eq!(plain.decode("library/ab", &open, 7).unwrap(), record);
    assert!(sealed.rejected().is_empty());
}

#[test]
fn server_sent_events_drop_overlong_lines() {
    let mut parser = Parser::default();
    assert!(parser.feed(&vec![b'x'; 200_000]).is_err());
    let mut parser = Parser::default();
    assert!(
        parser
            .feed(
                &[b"data: ".as_slice(), &vec![b'x'; 40_000], b"\ndata: ", &vec![b'x'; 40_000], b"\n"]
                    .concat()
            )
            .is_err()
    );
    let events = Parser::default().feed(b"event: seq\ndata: {\"seq\": 1}\n\n").unwrap();
    assert_eq!(events, [("seq".to_string(), json!({"seq": 1}))]);
}

struct Stuck;

impl Remote for Stuck {
    async fn pull(&self, since: i64) -> Result<Page> {
        Ok(Page { seq: 9, more: true, last: since, changes: Vec::new() })
    }

    async fn push(&self, _: &[(Key, i64, Doc)]) -> Result<Vec<Outcome>> {
        Ok(Vec::new())
    }
}

#[tokio::test]
async fn a_server_that_sends_no_progress_stops_the_sync() {
    let mut replica = Replica { seq: 3, ..Replica::default() };
    assert!(
        crate::sync(&Stuck, &mut replica, &Default::default(), |_| Rules::default(), &at(1, "a"))
            .await
            .is_err()
    );
    assert_eq!(replica.seq, 3);
}

#[tokio::test]
async fn answers_over_their_limit_are_refused() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for (n, stream) in listener.incoming().take(3).enumerate() {
            let mut stream = stream.unwrap();
            let _ = std::io::Read::read(&mut stream, &mut [0; 4096]);
            let body = format!("{{\"pad\": \"{}\"}}", "x".repeat(2 << 20));
            let length = if n == 0 { String::new() } else { format!("content-length: {}\r\n", body.len()) };
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n{length}connection: close\r\n\r\n"
            );
            let _ = std::io::Write::write_all(&mut stream, format!("{head}{body}").as_bytes());
        }
    });
    let api = Api::new(&url).unwrap();
    for _ in 0..2 {
        let refused = api.get("/api/v1/me").await.unwrap_err().to_string();
        assert!(refused.contains("over 1024 KiB"), "{refused}");
    }
    assert_eq!(
        api.fetch("/api/v1/sync/changes", PAGE).await.unwrap()["pad"].as_str().map(str::len),
        Some(2 << 20)
    );
}

fn pages(bodies: Vec<serde_json::Value>) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for (stream, body) in listener.incoming().zip(bodies) {
            let mut stream = stream.unwrap();
            let _ = std::io::Read::read(&mut stream, &mut [0; 4096]);
            let body = body.to_string();
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            let _ = std::io::Write::write_all(&mut stream, format!("{head}{body}").as_bytes());
        }
    });
    url
}

#[tokio::test]
async fn a_key_change_during_a_pull_stops_the_sync_without_quarantine() {
    let key = SyncKey::generate();
    let record = doc(&[("title", json!("Dawn"), 5)]);
    let newer = Session::new(Api::new("http://127.0.0.1:9").unwrap(), 3, 2, true, key.rotated());
    let payload = newer.encode("library/ab", &record).unwrap();
    let change = json!({"collection": "library", "item": "ab", "seq": 4, "epoch": 2, "deleted": false, "payload": payload});
    let url = pages(vec![
        json!({"seq": 4, "epoch": 2, "more": false, "changes": [change.clone()]}),
        json!({"seq": 4, "epoch": 1, "more": false, "changes": [change]}),
    ]);
    let session = Session::new(Api::new(&url).unwrap(), 3, 1, true, key);
    for _ in 0..2 {
        let mut replica = Replica { seq: 1, ..Replica::default() };
        let (local, now) = (Default::default(), at(9, "a"));
        let stopped = crate::sync(&session, &mut replica, &local, |_| Rules::default(), &now).await;
        assert_eq!(stopped.unwrap_err().code(), "epoch_mismatch");
        assert_eq!(replica.seq, 1);
    }
    assert!(session.rejected().is_empty());
}
