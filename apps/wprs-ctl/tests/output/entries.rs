use super::*;

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
        "features": [{"name": "inputs.pointer", "why": "follows the pointer"}], "languages": ["en", "es"],
        "source_url": "https://example.org/harbor"});
    let details = reply(json!({"item": item, "thumb": null, "presets": [], "credit": null, "revoked": null,
        "playlists": [], "installed": null, "about": about}));
    let shown = items::details(parse(&details).unwrap());
    let tail = "package = org.example.dawn 1.0.0 (image, CC0-1.0)\ncontent = mature (violence)\n\
        mature shown = false\nphotosensitive = yes, allowed\norigin = derived from Harbor by Ana\n\
        source = https://example.org/harbor\nauthors = Jo\ncredits = a.png: Bo (CC0-1.0)\n\
        links = homepage https://example.org\ninputs = pointer\n\
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
