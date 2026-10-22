mod fake;

use crate::args::{INVALID, carries_secret, fixture};
use crate::bus::{Daemon, url};
use crate::calls::{CALLS, dispatch};
use crate::grid::{Grid, Shape, compose, info, name};
use crate::{app, tr};
use serde_json::{Value, json};
use std::path::Path;
use wprs_core::model::*;

fn summary(uid: &str) -> ItemSummary {
    let mut page: SearchPage = wprs_client::parse(&fixture("Library1", "Search")).unwrap();
    ItemSummary { uid: uid.into(), ..page.items.remove(0) }
}

fn qml() -> Vec<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("qml");
    let files = std::fs::read_dir(&dir).unwrap().chain(std::fs::read_dir(dir.join("facade")).unwrap());
    files
        .map(|f| f.unwrap().path())
        .filter(|p| p.is_file())
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect()
}

fn literals<'a>(text: &'a str, call: &str) -> Vec<&'a str> {
    text.split(call).skip(1).filter_map(|rest| rest.strip_prefix('"')?.split('"').next()).collect()
}

#[test]
fn filters_compose_one_query() {
    assert_eq!(
        compose(" harbour ", "image", true, "night sky"),
        "kind:image fav:yes tag:\"night sky\" harbour"
    );
    assert_eq!(compose("", "", false, "sea"), "tag:sea");
    assert_eq!(compose("", "", false, ""), "");
}

#[test]
fn reloads_keep_thumbnails_and_selection() {
    let mut grid = Grid { size: 256, ..Grid::default() };
    grid.store(0, 3, vec![summary("A"), summary("B")]);
    assert!(matches!(grid.shape(2, &[summary("C")]), Shape::Append));
    grid.store(2, 3, vec![summary("C")]);
    assert_eq!(grid.wanted(0, 9), ["A", "B", "C"]);
    assert_eq!(grid.thumb("B", "/tmp/b.jpg"), [1]);
    assert_eq!(grid.toggle(1, true), [1]);
    assert_eq!(grid.toggle(2, false), [2]);
    assert!(matches!(grid.shape(0, &[summary("A"), summary("B")]), Shape::Shrink(2, 3)));
    grid.store(0, 2, vec![summary("A"), summary("B")]);
    assert_eq!((grid.rows[1].thumb.as_deref(), grid.rows[1].selected), (Some("/tmp/b.jpg"), true));
    assert_eq!(grid.selected().map(|i| i.uid.as_str()).collect::<Vec<_>>(), ["B"]);
    assert!(matches!(grid.shape(0, &[summary("B")]), Shape::Reset));
    assert_eq!(grid.toggle(usize::MAX, true), [1]);
}

#[test]
fn rows_describe_items() {
    let video = ItemSummary { kind: ItemKind::Video, duration_ms: Some(65000), ..summary("A") };
    assert_eq!(info(&video), "3840×2160 · 1:05");
    assert_eq!((name(ItemKind::Package), name(Reason::Takedown)), ("package".into(), "takedown".into()));
}

#[test]
fn secrets_never_travel_through_the_generic_call() {
    let add = |secret: &str| vec![json!("nas"), json!("webdav"), json!({}), json!(secret)];
    assert!(carries_secret("io.github.wallpaper_rs.Providers1", "SetSecret", &[json!("nas"), json!("x")]));
    assert!(carries_secret("Providers1", "Add", &add("hunter2")));
    assert!(!carries_secret("Providers1", "Add", &add("")));
    assert!(!carries_secret("Library1", "Add", &add("x")));
}

#[test]
fn file_uris_become_paths() {
    assert_eq!(app::path("file:///tmp/a%20b.png").as_deref(), Some("/tmp/a b.png"));
    assert_eq!(app::path("/tmp/c.png").as_deref(), Some("/tmp/c.png"));
    assert_eq!([app::path("https://example.org/a.png"), app::path("file://example.org/a.png")], [None, None]);
    assert_eq!(url("/tmp/a b#1.png"), "file:///tmp/a%20b%231.png");
}

#[test]
fn catalogs_are_parsed_and_chosen_by_locale() {
    let po = "msgid \"\"\nmsgstr \"x\"\n\nmsgid \"A \\\"b\\\"\"\nmsgstr \"\"\n\"C\\n\"\n\n#, fuzzy\nmsgid \"D\"\nmsgstr \"E\"\n";
    let parsed = tr::parse(po);
    assert_eq!((parsed.len(), parsed["A \"b\""].as_str()), (1, "C\n"));
    assert_eq!(tr::catalog(&["es_ES.UTF-8".into()])["Library"], "Biblioteca");
    assert!(tr::catalog(&["de_DE".into()]).is_empty());
}

#[test]
fn every_qml_string_has_a_spanish_translation() {
    let spanish = tr::catalog(&["es".into()]);
    for text in qml().iter().flat_map(|file| literals(file, "App.tr(")) {
        let translated = spanish.get(text).unwrap_or_else(|| panic!("no Spanish translation for {text:?}"));
        let marks = |s: &str| (1..4).filter(|n| s.contains(&format!("%{n}"))).count();
        assert_eq!(marks(text), marks(translated), "{text:?}");
    }
}

#[test]
fn qml_calls_name_members_of_the_call_table() {
    for line in qml().iter().flat_map(|file| file.lines().map(String::from).collect::<Vec<_>>()) {
        for member in literals(&line, "App.query(").into_iter().chain(literals(&line, "App.act(")) {
            let iface = ["Library1", "Playlists1", "Profiles1", "Providers1", "Repos1"];
            let iface = iface.into_iter().find(|i| line.contains(&format!("\"{i}\""))).unwrap_or("Daemon1");
            assert!(CALLS.iter().any(|(i, m, _)| (*i, *m) == (iface, member)), "{iface}.{member} in {line}");
        }
    }
}

#[tokio::test]
async fn calls_check_their_arguments() {
    let call = |member: &'static str, args: Value| async move {
        dispatch(&Daemon::Fixtures, "Library1", member, args.as_array().unwrap().clone()).await
    };
    assert_eq!(call("RequestThumbnails", json!([["A"], 256])).await.unwrap(), "[{}]");
    assert_eq!(call("SetRating", json!([["A"], 4])).await.unwrap(), "[]");
    for (member, args) in
        [("SetRating", json!([["A"]])), ("SetRating", json!(["A", 4])), ("Rescan", json!(["", 1]))]
    {
        assert_eq!(call(member, args).await.unwrap_err().name, INVALID);
    }
    assert_eq!(call("Resolve", json!([["A"]])).await.unwrap_err().message, "unknown method Library1.Resolve");
}

#[test]
fn fixtures_follow_the_data_model() {
    fn check<T: serde::de::DeserializeOwned>(member: &str) {
        let (iface, member) = member.split_once('.').unwrap();
        wprs_client::parse::<T>(&fixture(iface, member)).unwrap_or_else(|e| panic!("{iface}.{member}: {e}"));
    }
    check::<State>("Daemon1.GetState");
    check::<Schema>("Daemon1.GetSchema");
    check::<JobsReply>("Daemon1.ListJobs");
    check::<SearchPage>("Library1.Search");
    check::<TagsReply>("Library1.ListTags");
    check::<ItemDetails>("Library1.GetItem");
    check::<SourcesReply>("Library1.ListSources");
    check::<PlaylistsReply>("Playlists1.List");
    check::<PlaylistPage>("Playlists1.Get");
    check::<HistoryReply>("Playlists1.History");
    check::<ProfilesReply>("Profiles1.List");
    check::<ProfileReply>("Profiles1.Get");
    check::<PresetsReply>("Profiles1.ListPresets");
    check::<ProvidersReply>("Providers1.List");
    check::<ReposReply>("Repos1.List");
    check::<BrowsePage>("Repos1.Browse");
    check::<InstalledReply>("Repos1.ListInstalled");
}
