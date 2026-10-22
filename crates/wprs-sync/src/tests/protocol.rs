use super::{at, doc};
use crate::*;
use serde_json::json;
use std::sync::atomic::Ordering;
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
    assert!(parser.feed(b": keep-alive\n\nevent: se").is_empty());
    let events = parser.feed(b"q\ndata: {\"seq\": 4}\r\n\r\nevent: mail\ndata: {\"id\": 9}\n\n");
    assert_eq!(events, [("seq".to_string(), json!({"seq": 4})), ("mail".to_string(), json!({"id": 9}))]);
}

fn session(encrypted: bool, key: SyncKey) -> Session {
    let api = Api::new("http://127.0.0.1:9").unwrap();
    Session { api, user: 3, epoch: 1, encrypted, key, rejected: Default::default() }
}

#[test]
fn sessions_seal_records_only_for_their_account_and_place() {
    let key = SyncKey::generate();
    let copy = SyncKey::from_bytes(key.bytes()).unwrap();
    let (sealed, plain) = (session(true, key), session(false, SyncKey::generate()));
    let record = doc(&[("t:secret-tag", json!(true), 5)]);
    let payload = sealed.encode("library/ab", &record).unwrap();
    assert!(!payload.contains("secret"));
    assert_eq!(sealed.decode("library/ab", &payload).unwrap(), record);
    assert!(sealed.decode("library/cd", &payload).is_err());
    let mut other = session(true, copy);
    other.user = 4;
    assert!(other.decode("library/ab", &payload).is_err());
    let open = plain.encode("library/ab", &record).unwrap();
    assert_eq!(plain.decode("library/ab", &open).unwrap(), record);
    assert_eq!(sealed.rejected.load(Ordering::Relaxed), 0);
}

#[test]
fn server_sent_events_drop_overlong_lines() {
    let mut parser = Parser::default();
    assert!(parser.feed(&vec![b'x'; 200_000]).is_empty());
    let events = parser.feed(b"\n\nevent: seq\ndata: {\"seq\": 1}\n\n");
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
