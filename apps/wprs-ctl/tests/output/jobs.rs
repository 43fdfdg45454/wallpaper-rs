use super::*;

#[test]
fn finished_jobs_are_summarized_by_kind() {
    let scan = json!({"reports": [{"source": null, "added": 2, "updated": 1, "moved": 0, "missing": 3},
        {"source": "01J9Z0X9Z6Q7X4F3K5E1V0S001", "added": 1, "updated": 0, "moved": 1, "missing": 0}]});
    let sync = json!({"reports": [{"provider": "daily", "listed": 8, "downloaded": 2, "reused": 6, "skipped": 0,
        "failed": 0, "scan": {"source": null, "added": 2, "updated": 0, "moved": 0, "missing": 0}}]});
    let cases = [
        ("scan", scan, "3 added, 3 missing, 1 moved, 1 updated"),
        (
            "import",
            json!({"imported": [{"path": "/srv/example/a.png", "uid": "X"}], "failed": []}),
            "0 failed, 1 imported",
        ),
        ("analyze", json!({"analyzed": 5, "failed": 1}), "5 analyzed, 1 failed"),
        ("provider_sync", sync, "2 downloaded, 0 failed, 8 listed, 6 reused, 0 skipped"),
        ("repo_add", repo(), "Example wallpapers (2 packages)"),
        (
            "repo_refresh",
            json!({"repos": [repo()], "updates": [], "revoked": []}),
            "1 repos, 0 revoked, 0 updates",
        ),
        ("install", installed(), "org.example.dawn 1.0.0"),
    ];
    for (kind, output, detail) in cases {
        assert_eq!(show::summary(&job(kind, output)), format!("job 4 {kind} done: {detail}"));
    }
    assert_eq!(show::summary(&job("a_new_kind", json!({"anything": 1}))), "job 4 unknown done");
    let jobs = reply(json!({"jobs": [job("install", json!(null))]}));
    assert_eq!(
        show::jobs(parse(&jobs).unwrap()),
        format!(
            "JOB  KIND     STATE  PROGRESS    STARTED           TARGET  ERROR\n4    install  done   2/2 (100%)  {}\n",
            show::time(1)
        )
    );
}

#[test]
fn the_schema_lists_every_key_by_section() {
    let field = |key: &str, kind: &str, default: Value| {
        json!({"key": key, "kind": kind, "default": default, "min": null, "max": null, "step": null, "values": [],
            "required": false, "clearable": false})
    };
    let mut fps = field("playback.max_fps", "int", json!(30));
    fps.as_object_mut().unwrap().extend([("min".into(), json!(1)), ("max".into(), json!(240))]);
    let mut url = field("server", "url", Value::Null);
    url["required"] = json!(true);
    let kind = json!({"kind": "immich", "hosts": [], "options": [url], "secret": {"name": "api_key", "required": true}});
    let schema = reply(
        json!({"options": [fps], "rules": [field("synced", "bool", json!(false))], "auto": [],
        "preset": [], "providers": [kind], "query": [{"key": "tag", "hint": "a tag"}], "thumb_sizes": [256, 512],
        "fits": ["fill", "fit"]}),
    );
    assert_eq!(
        show::schema(parse(&schema).unwrap()),
        "options\nKEY               KIND  DEFAULT  RANGE   VALUES  FLAGS\nplayback.max_fps  int   30       1..240\n\n\
         rules\nKEY     KIND  DEFAULT  RANGE  VALUES  FLAGS\nsynced  bool  false\n\n\
         auto\nKEY  KIND  DEFAULT  RANGE  VALUES  FLAGS\n\n\
         preset\nKEY  KIND  DEFAULT  RANGE  VALUES  FLAGS\n\n\
         provider immich (secret: api_key; contacts: its server)\n\
         KEY     KIND  DEFAULT  RANGE  VALUES  FLAGS\nserver  url   -                       required\n\n\
         query\nKEY  MEANING\ntag  a tag\n\n\
         thumb_sizes = 256, 512\nfits = fill, fit\n"
    );
}

#[test]
fn small_values_read_well() {
    assert_eq!([0, 1023, 1024, 1572864].map(show::bytes), ["0 B", "1023 B", "1.0 KiB", "1.5 MiB"]);
    assert_eq!([(0, 0), (5, 0), (1, 4)].map(|(d, t)| show::progress(d, t)), ["0", "5", "1/4 (25%)"]);
    assert_eq!([show::short("01J9Z0X9Z6Q7X4F3K5E1V0R1AB"), show::prefix(REPO)], ["E1V0R1AB", "01234567"]);
    assert_eq!(items::dimensions(None, None, Some(3_725_000)), "62:05");
}

#[test]
fn control_characters_never_reach_the_terminal() {
    let title = "Dawn\u{1b}]52;c;AAAA\u{7}\r\nE1V0R2CD  fake";
    let page = reply(
        json!({"query": "", "total": 1, "offset": 0, "items": [summary("01J9Z0X9Z6Q7X4F3K5E1V0R1AB", title)]}),
    );
    let text = items::search(parse(&page).unwrap());
    assert!(text.contains("Dawn\u{fffd}]52;c;AAAA\u{fffd}\u{fffd}\u{fffd}E1V0R2CD  fake"), "{text}");
    assert_eq!(text.lines().count(), 4);
    assert_eq!(show::lines([("title", title.to_string())]).matches('\n').count(), 1);
}
