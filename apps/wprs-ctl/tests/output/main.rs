use serde_json::{Value, json};
use wprs_client::parse;
use wprs_core::model::Job;
use wprs_ctl::{items, pkg, playlists, profiles, providers, remote, show};

mod entries;
mod jobs;
mod lists;

fn reply(fields: Value) -> String {
    let mut reply = json!({"model": 1, "serial": 7});
    reply.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    reply.to_string()
}

fn summary(uid: &str, title: &str) -> Value {
    json!({"uid": uid, "kind": "image", "title": title, "width": 3840, "height": 2160, "duration_ms": null,
        "color": 3368703, "tags": ["sea", "blue"], "rating": 4, "favorite": true, "missing": false, "error": null,
        "revoked": null, "removable": false, "imported": false, "thumb": null})
}

fn rules() -> Value {
    json!({"order": "shuffle", "interval": "30m", "at": ["08:00", "20:00"], "days": [], "synced": false,
        "resume": true, "advance_while_paused": false, "skip_videos_on_battery": true, "transition_ms": null})
}

fn item() -> Value {
    json!({"uid": "01J9Z0X9Z6Q7X4F3K5E1V0R1AB", "source": null, "kind": "package", "path": "/srv/example/dawn.wpak",
        "title": "Dawn", "size": 1572864, "mtime": 1, "width": 1920, "height": 1080, "duration_ms": null, "audio": null,
        "color": 3368703, "error": null, "package": {"id": "org.example.dawn", "version": "1.0.0", "kind": "image",
        "license": "CC0-1.0"}, "tags": ["sky"], "rating": 0, "favorite": false, "added_at": 1, "used_at": null,
        "use_count": 0, "missing": false})
}

const REPO: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn repo() -> Value {
    json!({"repo_id": REPO, "url": "https://example.org/repo/?fingerprint=ab", "name": "Example wallpapers",
        "hosts": ["example.org"], "fingerprints": [], "root_version": 1, "root_expires": 0, "entry_version": 3,
        "entry_expires": 0, "checked": 0, "stale": true, "packages": 2, "warnings": ["stale", "root_expiring"],
        "last_error": null, "job": null})
}

fn installed() -> Value {
    json!({"repo_id": REPO, "id": "org.example.dawn", "version": "1.0.0", "sha256": "ab", "size": 1572864,
        "signer": null, "revoked": "license", "item": "01J9Z0X9Z6Q7X4F3K5E1V0R1AB", "update": "1.1.0"})
}

fn job(kind: &str, output: Value) -> Job {
    serde_json::from_value(
        json!({"id": 4, "kind": kind, "target": "", "state": "done", "done": 2, "total": 2,
        "started_at": 1, "finished_at": 2, "error": null, "output": output}),
    )
    .unwrap()
}
