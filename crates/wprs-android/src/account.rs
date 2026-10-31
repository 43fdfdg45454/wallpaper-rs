use crate::said;
use crate::state::{CANCEL, Saved, api, linked, read, update};
use crate::{Error, cycle, keys, link, pairing, settings, vault};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Mutex;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Duration;
use wprs_sync::Peer;

pub(crate) const NETWORK: &str = "sync_network";

static BUSY: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Request {
    Status,
    Cancel,
    Pause {
        paused: bool,
    },
    Add {
        url: String,
    },
    Link {
        label: String,
        plaintext: bool,
    },
    Wait,
    Join {
        device: String,
    },
    Share {
        device: String,
    },
    Confirm {
        code: String,
    },
    Preview {
        code: String,
    },
    Approve {
        code: String,
        approve: bool,
        #[serde(default)]
        password: String,
    },
    Sync {
        conflict: String,
    },
    Remove,
}

fn status() -> Value {
    read(|s| {
        let device = |p: &Peer| {
            let current = s.device.as_ref() == Some(&p.id);
            json!({"id": p.id, "name": p.name, "label": s.label_of(&p.id), "platform": p.platform,
                "seen_at": p.seen_at, "current": current})
        };
        json!({
            "url": s.url, "name": s.name, "handle": s.handle, "device": s.device_name, "label": s.label,
            "state": s.state(), "encrypted": s.encrypted, "paused": s.paused, "last_sync": s.last_sync,
            "last_error": s.last_error, "last_reply": s.last_reply, "warning": s.warning, "records": s.replica.bases.len(),
            "quarantined": s.quarantine.len(), "unapplied": s.unapplied.len(),
            "devices": s.peers.iter().map(device).collect::<Vec<_>>(), "link": link::shown(),
            "pairing": pairing::shown(), "network": settings::chosen(NETWORK), "active": s.active(),
            "outdated": s.outdated, "recovery_stale": s.recovery_stale,
        })
    })
}

pub(crate) fn request(text: &str) -> String {
    let answer = serde_json::from_str(text).map_err(Error::from).and_then(run);
    answer.unwrap_or_else(|e| e.reply()).to_string()
}

fn run(request: Request) -> Result<Value, Error> {
    match request {
        Request::Status => return Ok(status()),
        Request::Cancel => {
            CANCEL.store(true, Relaxed);
            return Ok(Value::Null);
        }
        Request::Pause { paused } => {
            linked()?;
            return update(|s| s.paused = paused).map(|()| status());
        }
        _ => {}
    }
    let _busy = BUSY.try_lock().map_err(|_| {
        said("android.another_account_task", "another account task is running; try again when it ends", &[])
    })?;
    CANCEL.store(false, Relaxed);
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(async {
        match request {
            Request::Add { url } => link::add(&url).await,
            Request::Link { label, plaintext } => link::start(&label, plaintext).await,
            Request::Wait => link::wait().await,
            Request::Join { device } => pairing::join(&device).await,
            Request::Share { device } => pairing::share(&device).await,
            Request::Confirm { code } => pairing::confirm(&code).await,
            Request::Preview { code } => keys::device_code(&code, "preview", json!({})).await,
            Request::Approve { code, approve, password } => {
                keys::device_code(&code, "approve", json!({"approve": approve, "password": password})).await
            }
            Request::Sync { conflict } => cycle::sync(&conflict).await,
            Request::Remove => remove().await,
            Request::Status | Request::Cancel | Request::Pause { .. } => Ok(Value::Null),
        }
    })
}

async fn remove() -> Result<Value, Error> {
    let device = read(|s| s.device.clone());
    if let (Some(device), Ok((api, _))) = (device, api()) {
        let path = format!("/api/v1/me/devices/{device}");
        let gone = api.call(reqwest::Method::DELETE, &path, None);
        if !matches!(tokio::time::timeout(Duration::from_secs(10), gone).await, Ok(Ok(_))) {
            log::warn!("the server could not be told to forget this phone");
        }
    }
    vault::forget()?;
    link::cancel();
    pairing::cancel();
    update(|s| *s = Saved::default())?;
    Ok(status())
}
