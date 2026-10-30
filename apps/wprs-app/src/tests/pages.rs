use crate::app::{Application, PAGES, page, pass_on};
use crate::bus::{self, Event};
use crate::tests::private_bus;
use std::collections::HashMap;
use std::path::Path;
use wprs_core::dbus::{APP_ID, APP_PATH};
use zbus::Variant;
use zbus::connection::Builder;

fn read(path: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap()
}

fn shown_by(page: &str) -> String {
    let file = |name: &str| Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("qml/{name}.qml"));
    let (mut names, mut text, mut at) = (vec![page.to_owned()], String::new(), 0);
    while let Some(name) = names.get(at).cloned() {
        let qml = std::fs::read_to_string(file(&name)).unwrap();
        for used in qml.split('{').filter_map(|chunk| chunk.split_whitespace().last()) {
            let new = used.starts_with(char::is_uppercase) && !names.iter().any(|n| n == used);
            if new && file(used).is_file() {
                names.push(used.to_owned());
            }
        }
        text += &qml;
        at += 1;
    }
    text
}

#[test]
fn pages_match_the_window_the_launcher_and_the_tray() {
    let names = PAGES.map(|page| format!("\"{page}\"")).join(", ");
    assert!(read("qml/Main.qml").contains(&format!("names: [{names}]")));
    let desktop = read("../../packaging/desktop/io.github.wallpaper_rs.desktop");
    assert!(desktop.contains("Actions=settings;\n\n[Desktop Action settings]\n"));
    assert!(desktop.contains("\nExec=@BINDIR@/wprs-app --page settings\n"));
    assert!(read("../../integrations/plasma-tray/contents/ui/Panel.qml").contains("openApp(\"settings\")"));
    assert_eq!([page(Some("settings")), page(Some("Settings")), page(None)], [Some("settings"), None, None]);
}

#[test]
fn the_tray_leaves_every_advanced_setting_to_the_settings_page() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../integrations/plasma-tray/contents/ui");
    let tray: String = std::fs::read_dir(dir)
        .unwrap()
        .map(|f| std::fs::read_to_string(f.unwrap().path()).unwrap())
        .collect();
    let settings = shown_by("SettingsPage");
    assert!(settings.contains("App.outputs.concat([\"\"])"));
    for call in ["SetBackground", "Start", "Stop", "Previous", "GetSceneProperties", "Every screen"] {
        let quoted = format!("\"{call}\"");
        assert!(!tray.contains(&quoted) && settings.contains(&quoted), "{call}");
    }
    let options: Vec<&str> = tray.split("\"playback.").skip(1).filter_map(|s| s.split('"').next()).collect();
    assert_eq!(options, ["scene_sound", "scene_capture"], "the tray sets only the scene audio switches");
    assert!(settings.contains("\"SetOption\""));
    for call in [
        "SetPaused",
        "SetMuted",
        "SetOption",
        "SetVolume",
        "Activate",
        "Next",
        "SetWallpaper",
        "SetFavorite",
        "Confirm",
    ] {
        assert!(tray.contains(&format!("act(\"{call}\"")), "{call}");
    }
}

#[test]
fn the_tray_offers_the_app_only_where_it_is_installed() {
    let flatpak = read("../../packaging/flatpak/io.github.wallpaper_rs.WallpaperRs.yml");
    assert!(flatpak.contains("integrations/plasma-tray") && !flatpak.contains(&format!("{APP_ID}.service")));
    let tray = read("../../integrations/plasma-tray/contents/ui/Daemon.qml");
    assert!(tray.contains(&format!("const app = \"{APP_ID}\"")) && tray.contains("\"ListActivatableNames\""));
    let panel = read("../../integrations/plasma-tray/contents/ui/Panel.qml");
    assert_eq!(panel.matches("visible: panel.daemon.hasApp\n").count(), 2);
    assert!(panel.contains("visible: !panel.daemon.hasApp\n"));
}

#[tokio::test]
async fn page_requests_reach_the_window_even_before_it_listens() {
    let (mut daemon, _dir, address) = private_bus();
    let app = Builder::address(address.as_str()).name(APP_ID).serve_at(APP_PATH, Application);
    let _app = app.build().await.unwrap();
    let other = Builder::address(address.as_str()).build().await.unwrap();
    pass_on(&other, Vec::new(), Some("settings")).await.unwrap();
    assert_eq!([bus::requested(), bus::requested()], [Some("settings".into()), None]);
    let mut events = bus::events();
    pass_on(&other, Vec::new(), Some("library")).await.unwrap();
    pass_on(&other, Vec::new(), None).await.unwrap();
    let unknown = ("nope", Vec::<Variant>::new(), HashMap::<&str, Variant>::new());
    let iface = Some("org.freedesktop.Application");
    other.call_method(Some(APP_ID), APP_PATH, iface, "ActivateAction", &unknown).await.unwrap();
    let mut pages = Vec::new();
    while pages.len() < 3 {
        if let Event::Activate(page) = events.recv().await.unwrap() {
            pages.push(page);
        }
    }
    assert_eq!(pages, ["library", "", ""]);
    assert_eq!(bus::requested().as_deref(), Some("library"));
    daemon.kill().ok();
    daemon.wait().ok();
}
