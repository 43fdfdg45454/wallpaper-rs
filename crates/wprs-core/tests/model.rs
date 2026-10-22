use serde_json::{Value, json};
use wprs_core::model::{Credit, Header, ItemKind, Job, JobKind, JobState, MODEL, Reason, SearchPage, Topic};

#[test]
fn search_pages_round_trip() {
    let reply = json!({"model": 1, "serial": 1_792_368_000_000_042_u64, "query": "tag:sea kind:image",
        "total": 42, "offset": 0, "items": [{"uid": "01J9Z0X9Z6Q7X4F3K5E1V0R1AB", "kind": "image",
        "title": "Harbour", "width": 3840, "height": 2160, "duration_ms": null, "color": 3_368_703,
        "tags": ["sea"], "rating": 4, "favorite": true, "missing": false, "error": null, "revoked": null,
        "removable": false, "imported": false, "thumb": "/thumbs/256/9f1c.jpg", "package": null}]});
    let page: SearchPage = serde_json::from_value(reply.clone()).unwrap();
    assert_eq!((page.model, page.total, page.items[0].kind), (MODEL, 42, ItemKind::Image));
    assert_eq!(serde_json::to_value(&page).unwrap(), reply);
    let header: Header = serde_json::from_value(reply).unwrap();
    assert_eq!(header.serial, 1_792_368_000_000_042);
}

#[test]
fn unknown_values_and_fields_are_tolerated() {
    let job = json!({"id": 3, "kind": "render", "target": "", "state": "paused", "done": 0, "total": 0,
        "started_at": 1, "finished_at": null, "error": null, "output": null, "added_later": true});
    let job: Job = serde_json::from_value(job).unwrap();
    assert_eq!((job.kind, job.state, job.output), (JobKind::Unknown, JobState::Unknown, None));
    let read = |v: &str| -> JobKind { serde_json::from_value(Value::String(v.into())).unwrap() };
    assert_eq!([read("provider_sync"), read("repo_add")], [JobKind::ProviderSync, JobKind::RepoAdd]);
    let topics: Vec<Topic> = serde_json::from_value(json!(["jobs", "weather"])).unwrap();
    assert_eq!(topics, [Topic::Jobs, Topic::Unknown]);
    assert_eq!(serde_json::from_value::<Reason>(json!("takedown")).unwrap(), Reason::Takedown);
}

#[test]
fn every_field_is_written_and_none_is_null() {
    let credit = Credit {
        provider: "daily".into(),
        title: "Harbour".into(),
        attribution: None,
        license: None,
        link: None,
    };
    let written =
        json!({"provider": "daily", "title": "Harbour", "attribution": null, "license": null, "link": null});
    assert_eq!(serde_json::to_value(credit).unwrap(), written);
    assert_eq!(serde_json::to_value(JobKind::RepoRefresh).unwrap(), json!("repo_refresh"));
}
