use crate::Error;
use crate::state::{api, read};
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::Duration;
use tokio::sync::Notify;
use tokio::time::timeout;
use wprs_sync::Parser;

static HUSH: Notify = Notify::const_new();
static HUSHED: AtomicU64 = AtomicU64::new(0);
const SILENT: Duration = Duration::from_secs(60);
const QUIET: &str = r#"{"changed": false, "ended": false}"#;

pub(crate) fn hush(token: u64) {
    HUSHED.store(token, Relaxed);
    HUSH.notify_waiters();
}

fn worth(event: &str, value: &Value) -> bool {
    let behind = value["seq"].as_i64().is_some_and(|seq| seq > read(|s| s.replica.seq));
    (event == "seq" && behind) || event == "epoch" || (event == "mail" && read(|s| s.stale || !s.ready))
}

pub(crate) async fn listen(token: u64) -> Result<Value, Error> {
    let quiet = || Ok(serde_json::from_str(QUIET)?);
    let hushed = HUSH.notified();
    tokio::pin!(hushed);
    if HUSHED.load(Relaxed) == token {
        return quiet();
    }
    if !crate::state::active() {
        return Ok(json!({"changed": false, "ended": true}));
    }
    let (api, _) = api()?;
    let opened = tokio::select! {
        opened = timeout(Duration::from_secs(30), api.events()) => opened,
        () = &mut hushed => return quiet(),
    };
    let silent = || crate::said("android.events_silent", "the server event stream did not answer", &[]);
    let mut response = opened.map_err(|_| silent())??;
    let state = api.get("/api/v1/sync/state").await?;
    if worth("seq", &json!({"seq": state["seq"]})) || state["epoch"].as_i64() > Some(read(|s| s.epoch)) {
        return Ok(json!({"changed": true, "ended": false}));
    }
    let mut parser = Parser::default();
    loop {
        let chunk = tokio::select! {
            chunk = timeout(SILENT, response.chunk()) => chunk,
            () = &mut hushed => return quiet(),
        };
        let Ok(Ok(Some(chunk))) = chunk else { return Ok(json!({"changed": false, "ended": true})) };
        let events = parser.feed(&chunk).map_err(Error::from)?;
        if events.iter().any(|(event, value)| worth(event, value)) {
            return Ok(json!({"changed": true, "ended": false}));
        }
    }
}
