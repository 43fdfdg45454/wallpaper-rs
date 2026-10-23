mod depth;
mod editor;
mod fake;
mod pages;

use crate::args::{INVALID, carries_secret, fixture};
use crate::bus::{Daemon, url};
use crate::calls::{CALLS, dispatch};
use crate::diff::Edit;
use crate::grid::{Grid, compose, info, kind, name};
use crate::{app, tr};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use tempfile::TempDir;
use wprs_core::model::*;

fn summary(uid: &str) -> ItemSummary {
    let mut page: SearchPage = wprs_client::parse(&fixture("Library1", "Search")).unwrap();
    ItemSummary { uid: uid.into(), ..page.items.remove(0) }
}

pub fn qml() -> Vec<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("qml");
    let files = std::fs::read_dir(&dir).unwrap().chain(std::fs::read_dir(dir.join("facade")).unwrap());
    files
        .map(|f| f.unwrap().path())
        .filter(|p| p.is_file())
        .map(|p| std::fs::read_to_string(p).unwrap())
        .collect()
}

pub fn private_bus() -> (Child, TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    let listen = format!("<listen>unix:dir={}</listen><auth>EXTERNAL</auth>", dir.path().display());
    let policy = r#"<policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy>"#;
    std::fs::write(dir.path().join("bus.conf"), format!("<busconfig>{listen}{policy}</busconfig>")).unwrap();
    let mut bus = Command::new("dbus-daemon")
        .args(["--nofork", "--print-address", "--config-file"])
        .arg(dir.path().join("bus.conf"))
        .stdout(Stdio::piped())
        .spawn()
        .expect("dbus-daemon is installed");
    let mut address = String::new();
    BufReader::new(bus.stdout.take().unwrap()).read_line(&mut address).unwrap();
    (bus, dir, address.trim().to_owned())
}

fn iface(line: &str) -> &'static str {
    let ifaces = ["Library1", "Playlists1", "Profiles1", "Providers1", "Repos1", "Inputs1", "Sync1"];
    ifaces.into_iter().find(|i| line.contains(&format!("\"{i}\""))).unwrap_or("Daemon1")
}

pub fn literals<'a>(text: &'a str, call: &str) -> Vec<&'a str> {
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

fn reload(grid: &mut Grid, uids: &[&str]) -> Vec<Edit> {
    let edits = grid.plan(uids.iter().map(|uid| summary(uid)).collect());
    edits.iter().for_each(|edit| grid.apply(edit));
    edits
}

#[test]
fn reloads_update_rows_in_place() {
    let mut grid = Grid { size: 256, ..Grid::default() };
    grid.apply(&Edit::Insert(0, vec![summary("A"), summary("B"), summary("C")]));
    assert_eq!(grid.wanted(0, 9), ["A", "B", "C"]);
    assert_eq!((grid.thumb("B", "/tmp/b.jpg"), grid.toggle(1, true)), (vec![1], vec![1]));
    let renamed = ItemSummary { title: "Renamed".into(), ..summary("A") };
    let edits = grid.plan(vec![summary("N"), summary("M"), summary("B"), renamed.clone()]);
    let expected = [
        Edit::Remove(2, 3),
        Edit::Insert(0, vec![summary("N"), summary("M")]),
        Edit::Move(3, 2),
        Edit::Set(3, Box::new(renamed)),
    ];
    assert_eq!(edits, expected);
    edits.iter().for_each(|edit| grid.apply(edit));
    let uids: Vec<_> = grid.rows.iter().map(|r| r.item.uid.as_str()).collect();
    assert_eq!((uids, grid.rows[3].item.title.as_str()), (vec!["N", "M", "B", "A"], "Renamed"));
    assert_eq!((grid.rows[2].thumb.as_deref(), grid.rows[2].selected), (Some("/tmp/b.jpg"), true));
    assert_eq!(reload(&mut grid, &["N", "M", "B", "A"]), [Edit::Set(3, Box::new(summary("A")))]);
    assert!(reload(&mut grid, &["N", "M", "B", "A"]).is_empty());
    let twice =
        [Edit::Remove(3, 4), Edit::Remove(1, 2), Edit::Move(1, 0), Edit::Insert(1, vec![summary("B")])];
    assert_eq!(reload(&mut grid, &["B", "B", "N"]), twice);
    assert_eq!(reload(&mut grid, &["B"]), [Edit::Remove(2, 3), Edit::Remove(1, 2)]);
    assert_eq!((grid.rows.len(), grid.rows[0].selected), (1, true));
}

#[test]
fn changed_items_refresh_their_thumbnails() {
    let mut grid = Grid { size: 256, ..Grid::default() };
    grid.apply(&Edit::Insert(0, vec![summary("A"), summary("B")]));
    grid.thumb("A", "/tmp/a.jpg");
    grid.thumb("B", "/tmp/b.jpg");
    assert!(grid.wanted(0, 9).is_empty());
    grid.stale(&["B".into()]);
    assert_eq!(grid.wanted(0, 9), ["B"]);
    assert_eq!(grid.thumb("B", "/tmp/b.jpg"), Vec::<usize>::new());
    grid.stale(&[]);
    assert_eq!(grid.wanted(0, 9), ["A", "B"]);
    assert_eq!(grid.thumb("A", "/tmp/a2.jpg"), [0]);
    let cached = ItemSummary { thumb: Some("/tmp/b3.jpg".into()), ..summary("B") };
    grid.apply(&Edit::Set(1, Box::new(cached)));
    assert_eq!(grid.rows[1].thumb.as_deref(), Some("/tmp/b3.jpg"));
    assert!(grid.wanted(0, 9).is_empty());
    assert_eq!(grid.toggle(usize::MAX, true), Vec::<usize>::new());
}

#[test]
fn rows_describe_items() {
    let video = ItemSummary { kind: ItemKind::Video, duration_ms: Some(65000), ..summary("A") };
    assert_eq!(info(&video), "3840×2160 · 1:05");
    assert_eq!((name(ItemKind::Package), name(Reason::Takedown)), ("package".into(), "takedown".into()));
    let page: SearchPage = wprs_client::parse(&fixture("Library1", "Search")).unwrap();
    assert_eq!(
        page.items.iter().map(kind).collect::<Vec<_>>(),
        ["image", "video", "package", "image", "scene"]
    );
}

#[test]
fn secrets_never_travel_through_the_generic_call() {
    let add = |secret: &str| vec![json!("nas"), json!("webdav"), json!({}), json!(secret)];
    assert!(carries_secret("io.github.wallpaper_rs.Providers1", "SetSecret", &[json!("nas"), json!("x")]));
    assert!(carries_secret("Providers1", "Add", &add("hunter2")));
    assert!(!carries_secret("Providers1", "Add", &add("")));
    assert!(!carries_secret("Library1", "Add", &add("x")));
    assert!(carries_secret("Sync1", "SetEscrow", &[json!("copper lantern drifts over harbor")]));
    assert!(carries_secret("Sync1", "Restore", &[json!("copper lantern drifts over harbor")]));
    assert!(!carries_secret("Sync1", "SetEscrow", &[json!("")]));
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
    let parsed = wprs_core::locale::parse(po);
    assert_eq!((parsed.len(), parsed["A \"b\""].as_str()), (1, "C\n"));
    assert_eq!(tr::catalog(&["es_ES.UTF-8".into()])["Library"], "Biblioteca");
    assert!(tr::catalog(&["de_DE".into()]).is_empty());
}

#[test]
fn every_qml_string_has_a_spanish_translation() {
    let spanish = tr::catalog(&["es".into()]);
    for text in qml().iter().flat_map(|file| literals(file, ".tr(")) {
        let translated = spanish.get(text).unwrap_or_else(|| panic!("no Spanish translation for {text:?}"));
        let marks = |s: &str| (1..4).filter(|n| s.contains(&format!("%{n}"))).count();
        assert_eq!(marks(text), marks(translated), "{text:?}");
    }
}

#[test]
fn package_texts_render_as_plain_text() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("qml");
    let data = ["modelData", "title", "info", "credit", "item", "details", "name", "playing", "tags"];
    let shown = |line: &str| data.iter().any(|d| line.contains(d));
    let files = ["PackageAbout", "Browse", "Tile", "PlaylistItems", "ItemDrawer", "Installed", "OutputRow"];
    let files = files
        .into_iter()
        .chain(["SceneField", "History", "RepoList", "Thumbnail"])
        .map(|name| dir.join(name));
    let tray = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../integrations/plasma-tray/contents/ui");
    for name in files.chain(["ScreenRow", "Header", "Thumbnail"].map(|name| tray.join(name))) {
        let text = std::fs::read_to_string(name.with_extension("qml")).unwrap();
        let name = name.display();
        let tips = text.lines().map(str::trim_start).filter(|l| l.contains("ToolTip.text:"));
        tips.for_each(|tip| assert!(!shown(tip), "{name}: {tip}"));
        let kinds = [".Label {", "Kirigami.Heading {", "QQC2.Switch {", "QQC2.CheckBox {"];
        for (at, kind) in kinds.iter().flat_map(|k| text.match_indices(k)) {
            let block = &text[at..];
            let mut depth = 0;
            let end = block.char_indices().find_map(|(i, c)| {
                depth += i32::from(c == '{') - i32::from(c == '}');
                (c == '}' && depth == 0).then_some(i)
            });
            let block = &block[..end.unwrap()];
            let text = block.lines().find(|l| l.trim_start().starts_with("text:")).unwrap_or_default();
            let plain = block.contains("textFormat:") && !kind.contains("Switch") && !kind.contains("Check");
            assert!(!shown(text) || plain, "{name}: {block}");
            assert!(!block.contains("StyledText") || block.contains("App.markup("), "{name}: {block}");
        }
    }
}

#[test]
fn qml_calls_name_members_of_the_call_table() {
    for line in qml().iter().flat_map(|file| file.lines().map(String::from).collect::<Vec<_>>()) {
        for member in literals(&line, ".query(").into_iter().chain(literals(&line, ".act(")) {
            let iface = iface(&line);
            assert!(CALLS.iter().any(|(i, m, _)| (*i, *m) == (iface, member)), "{iface}.{member} in {line}");
        }
    }
}

#[test]
fn tray_calls_carry_their_signatures() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../integrations/plasma-tray/contents/ui");
    let files = std::fs::read_dir(dir).unwrap().map(|f| std::fs::read_to_string(f.unwrap().path()).unwrap());
    let text: String = files.collect();
    for line in text.lines() {
        for member in literals(line, "act(").into_iter().chain(literals(line, "query(")) {
            let iface = iface(line);
            let call = CALLS.iter().find(|(i, m, _)| (*i, *m) == (iface, member));
            let (_, _, args) = call.unwrap_or_else(|| panic!("{iface}.{member} in {line}"));
            let signature =
                text.split(&format!("\"{iface}.{member}\": \"(")).nth(1).and_then(|s| s.split(')').next());
            assert_eq!(
                signature.unwrap_or_default().replace("as", "s").len(),
                args.len(),
                "{iface}.{member}"
            );
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
    check::<SceneProperties>("Daemon1.GetSceneProperties");
    check::<InputsReply>("Inputs1.GetInputs");
    check::<AccountReply>("Sync1.GetStatus");
    check::<FilesReply>("Sync1.ListFiles");
}
