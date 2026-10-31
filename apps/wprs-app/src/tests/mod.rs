mod depth;
mod editor;
mod fake;
mod grid;
mod pages;
mod picture;
mod replies;
mod tables;
mod words;

use crate::args::{INVALID, carries_secret, fixture};
use crate::bus::{Daemon, url};
use crate::calls::dispatch;
use crate::{app, tr};
use serde_json::{Value, json};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use tempfile::TempDir;
use wprs_core::model::*;

pub fn summary(uid: &str) -> ItemSummary {
    let mut page: SearchPage = wprs_client::parse(&fixture("Library1", "Search")).unwrap();
    ItemSummary { uid: uid.into(), ..page.items.remove(0) }
}

pub fn qml_files() -> Vec<(String, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("qml");
    let files = std::fs::read_dir(&dir).unwrap().chain(std::fs::read_dir(dir.join("facade")).unwrap());
    let files = files.map(|f| f.unwrap().path()).filter(|p| p.is_file());
    let name = |p: &Path| p.file_name().unwrap().to_string_lossy().into_owned();
    files.map(|p| (name(&p), std::fs::read_to_string(&p).unwrap())).collect()
}

pub fn qml() -> Vec<String> {
    qml_files().into_iter().map(|(_, text)| text).collect()
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

pub fn iface(line: &str) -> &'static str {
    let ifaces = ["Library1", "Playlists1", "Profiles1", "Providers1", "Repos1", "Inputs1", "Sync1"];
    ifaces.into_iter().find(|i| line.contains(&format!("\"{i}\""))).unwrap_or("Daemon1")
}

pub fn literals<'a>(text: &'a str, call: &str) -> Vec<&'a str> {
    text.split(call).skip(1).filter_map(|rest| rest.strip_prefix('"')?.split('"').next()).collect()
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
    assert!(carries_secret("Sync1", "ApproveDevice", &[json!("BCDF-GHJK"), json!("correct horse")]));
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
    let files = ["PackageAbout", "Browse", "Tile", "PlaylistItems", "ItemDrawer", "ItemFacts", "Installed"];
    let files = files
        .into_iter()
        .chain([
            "OutputRow",
            "OutputControls",
            "SceneField",
            "SceneEntry",
            "ScenePresets",
            "SavedPresets",
            "History",
            "RepoList",
            "Thumbnail",
        ])
        .map(|name| dir.join(name));
    let tray = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../integrations/plasma-tray/contents/ui");
    for name in files.chain(["ScreenRow", "Header", "Thumbnail"].map(|name| tray.join(name))) {
        let text = std::fs::read_to_string(name.with_extension("qml")).unwrap();
        let name = name.display();
        let tips = text.lines().map(str::trim_start).filter(|l| l.contains("ToolTip.text:"));
        tips.for_each(|tip| assert!(!shown(tip), "{name}: {tip}"));
        let kinds =
            [".Label {", "Kirigami.Heading {", "QQC2.Switch {", "QQC2.CheckBox {", "QQC2.ItemDelegate {"];
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
