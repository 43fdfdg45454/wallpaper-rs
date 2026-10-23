use serde_json::{Value, json};
use wprs_client::parse;
use wprs_core::model::Job;
use wprs_ctl::{items, pkg, playlists, profiles, providers, remote, show};

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

#[test]
fn search_pages_show_the_normalized_query_and_aligned_items() {
    let waves = json!({"uid": "01J9Z0X9Z6Q7X4F3K5E1V0R9ZZ", "kind": "video", "title": "Waves", "width": 1920,
        "height": 1080, "duration_ms": 95000, "color": null, "tags": [], "rating": 0, "favorite": false,
        "missing": true, "error": "cannot decode", "revoked": "malware", "removable": true, "imported": true,
        "thumb": null});
    let items = [summary("01J9Z0X9Z6Q7X4F3K5E1V0R1AB", "Harbour"), waves];
    let page = reply(json!({"query": "tag:sea sort:random:7", "total": 42, "offset": 2, "items": items}));
    assert_eq!(
        items::search(parse(&page).unwrap()),
        "query = tag:sea sort:random:7\n\
         items = 3-4 of 42\n\
         ITEM      KIND   SIZE            RATING  FLAGS                   TITLE    TAGS\n\
         E1V0R1AB  image  3840x2160       4       favorite                Harbour  sea, blue\n\
         E1V0R9ZZ  video  1920x1080 1:35  0       missing,broken,revoked  Waves\n"
    );
}

#[test]
fn item_details_list_only_what_is_set() {
    let preset = json!({"uid": "01J9Z0X9Z6Q7X4F3K5E1V0P001", "item": "01J9Z0X9Z6Q7X4F3K5E1V0R1AB", "name": "dark",
        "value": {"fit": "fit", "background": "#000000"}});
    let installed = json!({"repo_id": REPO, "id": "org.example.dawn", "version": "1.0.0", "sha256": "ab",
        "size": 1572864, "signer": null, "revoked": null, "item": "01J9Z0X9Z6Q7X4F3K5E1V0R1AB", "update": null});
    let details =
        reply(json!({"item": item(), "thumb": null, "presets": [preset], "credit": null, "revoked": null,
        "playlists": [{"uid": "01J9Z0X9Z6Q7X4F3K5E1V0L001", "name": "Mornings"}], "installed": installed}));
    let expected = format!(
        "item = 01J9Z0X9Z6Q7X4F3K5E1V0R1AB\ntitle = Dawn\nkind = package\npath = /srv/example/dawn.wpak\n\
         size = 1.5 MiB\ndimensions = 1920x1080\ncolor = #3366ff\ntags = sky\nrating = 0\nfavorite = false\n\
         added = {}\npackage = org.example.dawn 1.0.0 (image, CC0-1.0)\ninstalled = 1.0.0 from 01234567\n\
         presets = dark (fit #000000)\nplaylists = Mornings\n",
        show::time(1)
    );
    assert_eq!(items::details(parse(&details).unwrap()), expected);
}

#[test]
fn package_details_add_their_metadata() {
    let mut item = item();
    item["package"]["content"] = json!("mature");
    item["package"]["photosensitive"] = json!(true);
    item["package"]["confirmed"] = json!(["photosensitive"]);
    let about = json!({"schema": "wallpaper-rs.wpak/2", "title": "Dawn", "description": "Hills at **dusk**.\n\n- one",
        "changes": "", "authors": ["Jo"], "license": "CC0-1.0", "content": "mature", "descriptors": ["violence"],
        "photosensitive": true, "origin": "derived",
        "derived_from": [{"author": "Ana", "title": "Harbor", "url": null, "license": null}],
        "credits": [{"paths": ["a.png"], "license": "CC0-1.0", "attribution": "Bo", "url": null}],
        "links": [{"kind": "homepage", "url": "https://example.org"}], "inputs": ["pointer"],
        "features": [{"name": "inputs.pointer", "why": "follows the pointer"}], "languages": ["en", "es"]});
    let details = reply(json!({"item": item, "thumb": null, "presets": [], "credit": null, "revoked": null,
        "playlists": [], "installed": null, "about": about}));
    let shown = items::details(parse(&details).unwrap());
    let tail = "package = org.example.dawn 1.0.0 (image, CC0-1.0)\ncontent = mature (violence)\n\
        mature shown = false\nphotosensitive = yes, allowed\norigin = derived from Harbor by Ana\nauthors = Jo\n\
        credits = a.png: Bo (CC0-1.0)\nlinks = homepage https://example.org\ninputs = pointer\n\
        needs = inputs.pointer (follows the pointer)\nlanguages = en, es\nschema = wallpaper-rs.wpak/2\n\n\
        Description:\nHills at dusk.\n\n• one\n";
    assert!(shown.ends_with(tail), "{shown}");
}

#[test]
fn sources_and_tags_are_tables() {
    let folder = json!({"uid": "01J9Z0X9Z6Q7X4F3K5E1V0S001", "path": "/srv/example/pictures", "recursive": true,
        "scanned_at": null, "error": null, "items": 120, "provider": null});
    let cache = json!({"uid": "01J9Z0X9Z6Q7X4F3K5E1V0S002", "path": "/srv/example/daily", "recursive": false,
        "scanned_at": null, "error": "permission denied", "items": 3, "provider": "daily"});
    assert_eq!(
        items::sources(parse(&reply(json!({"sources": [folder, cache]}))).unwrap()),
        "SOURCE    ITEMS  SUBFOLDERS  SCANNED  PROVIDER  PATH\n\
         E1V0S001  120    yes         -        -         /srv/example/pictures\n\
         E1V0S002  3      no          -        daily     /srv/example/daily (permission denied)\n"
    );
    let tags = reply(json!({"tags": [{"name": "sea", "count": 12}, {"name": "sky", "count": 3}]}));
    assert_eq!(items::tags(parse(&tags).unwrap()), "COUNT  TAG\n12     sea\n3      sky\n");
}

#[test]
fn playlists_show_their_rules_and_numbered_entries() {
    let manual = json!({"uid": "01J9Z0X9Z6Q7X4F3K5E1V0L001", "name": "Mornings", "query": null, "rules": rules(),
        "count": 2, "outputs": ["DP-1"]});
    let smart = json!({"uid": "01J9Z0X9Z6Q7X4F3K5E1V0L002", "name": "Sea", "query": "tag:sea", "rules": rules(),
        "count": 40, "outputs": []});
    assert_eq!(
        playlists::playlists(parse(&reply(json!({"playlists": [manual, smart]}))).unwrap()),
        "PLAYLIST  NAME      TYPE    COUNT  ORDER    INTERVAL  OUTPUTS\n\
         E1V0L001  Mornings  manual  2      shuffle  30m       DP-1\n\
         E1V0L002  Sea       smart   40     shuffle  30m\n"
    );
    let items = [summary("01J9Z0X9Z6Q7X4F3K5E1V0R1AB", "Harbour")];
    let page = reply(json!({"playlist": manual, "total": 2, "offset": 1, "items": items}));
    assert_eq!(
        playlists::page(parse(&page).unwrap()),
        "playlist = 01J9Z0X9Z6Q7X4F3K5E1V0L001\nname = Mornings\noutputs = DP-1\nitems = 2 of 2\n\
         #  ITEM      KIND   SIZE       RATING  FLAGS     TITLE    TAGS\n\
         1  E1V0R1AB  image  3840x2160  4       favorite  Harbour  sea, blue\n"
    );
    let rules = playlists::rules(&serde_json::from_value(rules()).unwrap());
    assert_eq!(show::pick(rules.clone(), Some("at")).unwrap(), "08:00,20:00\n");
    assert_eq!(
        show::pick(rules.clone(), None).unwrap(),
        "order = shuffle\ninterval = 30m\nat = 08:00,20:00\ndays =\nsynced = false\nresume = true\n\
         advance_while_paused = false\nskip_videos_on_battery = true\ntransition_ms =\n"
    );
    assert!(
        show::pick(rules, Some("speed"))
            .unwrap_err()
            .to_string()
            .starts_with("unknown key speed; the keys are order")
    );
}

#[test]
fn profiles_and_presets_show_their_sections() {
    let profile = json!({"uid": "01J9Z0X9Z6Q7X4F3K5E1V0F001", "name": "Desk", "priority": 5, "active": true,
        "outputs": {"": {"item": null, "path": null, "playlist": "01J9Z0X9Z6Q7X4F3K5E1V0L001", "fit": "fill",
            "background": "#000000", "preset": null},
            "DP-1": {"item": "01J9Z0X9Z6Q7X4F3K5E1V0R1AB", "path": null, "playlist": null, "fit": null,
            "background": null, "preset": "01J9Z0X9Z6Q7X4F3K5E1V0P001"}},
        "options": {"pause.on_battery": "true"}, "auto": {"outputs": ["DP-1", "HDMI-A-1"], "on_battery": null}});
    let list = reply(json!({"active": "01J9Z0X9Z6Q7X4F3K5E1V0F001", "profiles": [profile]}));
    assert_eq!(
        profiles::list_profiles(parse(&list).unwrap()),
        "PROFILE   NAME  ACTIVE  PRIORITY  AUTO\nE1V0F001  Desk  yes     5         outputs=DP-1,HDMI-A-1\n"
    );
    assert_eq!(
        profiles::details(serde_json::from_value(profile).unwrap()),
        "profile = 01J9Z0X9Z6Q7X4F3K5E1V0F001\nname = Desk\nactive = true\noutputs = DP-1,HDMI-A-1\n\
         on_battery =\npriority = 5\noutput default = playlist E1V0L001, fill, #000000\n\
         output DP-1 = item E1V0R1AB, preset E1V0P001\noption pause.on_battery = true\n"
    );
    let preset = json!({"uid": "01J9Z0X9Z6Q7X4F3K5E1V0P002", "item": null, "name": "night",
        "value": {"fit": null, "background": "#101010"}});
    assert_eq!(
        profiles::presets(parse(&reply(json!({"item": null, "presets": [preset]}))).unwrap()),
        "PRESET    ITEM  NAME   FIT  BACKGROUND\nE1V0P002  -     night  -    #101010\n"
    );
}

#[test]
fn providers_and_repositories_show_their_state() {
    let provider = json!({"id": "photos", "kind": "immich", "options": {"server": "https://photos.example.org"},
        "source": null, "state": "needs_secret", "has_secret": false, "hosts": ["photos.example.org"], "items": 0,
        "cache_bytes": 0, "last_sync": null, "next_sync": null, "last_error": "the key was rejected", "job": null});
    assert_eq!(
        providers::list_providers(parse(&reply(json!({"providers": [provider]}))).unwrap()),
        "PROVIDER  KIND    STATE         ITEMS  CACHE  SYNCED  NEXT  ERROR\n\
         photos    immich  needs_secret  0      0 B    -       -     the key was rejected\n"
    );
    assert_eq!(
        remote::list(parse(&reply(json!({"repos": [repo()]}))).unwrap()),
        format!(
            "REPO      NAME                PACKAGES  CHECKED           EXPIRES           STATUS\n\
             01234567  Example wallpapers  2         {0}  {0}  stale, root_expiring\n",
            show::time(0)
        )
    );
    let package = json!({"repo_id": REPO, "repo_name": "Example wallpapers", "id": "org.example.dawn",
        "author": null, "version": "1.1.0", "kind": "image", "title": "Sky at dawn", "description": "", "tags": [],
        "license": "CC0-1.0", "authors": [], "size": 1024, "published": 0, "preview": true,
        "versions": ["1.0.0", "1.1.0"], "installed": ["1.0.0"], "update": true, "content": "general",
        "photosensitive": true});
    let page = reply(json!({"query": "dawn", "total": 1, "offset": 0, "packages": [package]}));
    assert_eq!(
        pkg::browse(parse(&page).unwrap()),
        "query = dawn\npackages = 1 of 1\n\
         REPO      ID                VERSION  KIND   CONTENT                  INSTALLED                 TITLE\n\
         01234567  org.example.dawn  1.1.0    image  general, photosensitive  1.0.0 (update available)  Sky at dawn\n"
    );
    assert_eq!(
        pkg::installed(parse(&reply(json!({"packages": [installed()]}))).unwrap()),
        "REPO      ID                VERSION  SIZE     ITEM      UPDATE  REVOKED\n\
         01234567  org.example.dawn  1.0.0    1.5 MiB  E1V0R1AB  1.1.0   license\n"
    );
}

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
