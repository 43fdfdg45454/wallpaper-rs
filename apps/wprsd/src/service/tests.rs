use super::*;
use zbus::fdo::Error::InvalidArgs;

#[test]
fn interface_name_matches_core_constant() {
    let literal = format!("#[zbus::interface(name = \"{}\")]", wprs_core::dbus::INTERFACE);
    assert!(include_str!("../service.rs").contains(&literal));
}

#[test]
fn only_methods_that_change_what_is_shown_need_the_renderer() {
    let guarded = |source: &str| -> Vec<String> {
        let code = source.split("#[cfg(test)]").next().unwrap_or_default();
        let uses = |body: &&str| [".render(", ".request("].iter().any(|call| body.contains(call));
        let methods = code.split("async fn ").skip(1).filter(uses);
        methods.map(|m| m.split('(').next().unwrap_or_default().to_owned()).collect()
    };
    let daemon = "request_stream hold_stream release_stream set_output_covered set_wallpaper set_background set_filter \
        press_scene_button set_option reload retry_effects set_paused set_volume set_muted";
    assert_eq!(guarded(include_str!("../service.rs")), daemon.split_whitespace().collect::<Vec<_>>());
    assert_eq!(guarded(include_str!("../library1.rs")), ["apply"]);
    assert_eq!(guarded(include_str!("../playlists1.rs")), ["start", "stop", "next", "previous"]);
    assert_eq!(guarded(include_str!("../profiles1.rs")), ["activate"]);
    for source in [include_str!("../providers1.rs"), include_str!("../repos1.rs")] {
        assert!(guarded(source).is_empty());
    }
    let (name, message) = crate::bus::rendering(false).unwrap_err().parts();
    assert_eq!(name, "org.freedesktop.DBus.Error.Failed");
    assert!(crate::bus::rendering(true).is_ok() && message.ends_with("restart wprsd"));
}

#[test]
fn only_local_files_are_wallpapers() {
    assert_eq!(local_path("file:///tmp/a%20b.png").ok(), Some("/tmp/a b.png".into()));
    assert_eq!(local_path("/tmp/a:b.png").ok(), Some("/tmp/a:b.png".into()));
    for uri in ["http://127.0.0.1:9/x.webm", "data:image/png;base64,AA", "file://elsewhere/x.png"] {
        assert!(matches!(local_path(uri), Err(InvalidArgs(_))), "{uri}");
    }
}
