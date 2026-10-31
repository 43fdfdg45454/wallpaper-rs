use super::*;

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
