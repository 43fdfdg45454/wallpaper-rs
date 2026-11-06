use super::qml_files;

#[test]
fn the_rgb_choice_while_covered_applies_only_while_the_covered_wallpaper_pauses() {
    let (_, text) = qml_files().into_iter().find(|(n, _)| n == "SettingsRgb.qml").unwrap();
    assert!(
        text.contains(r#"options["pause.when_covered"] !== false"#),
        "the choice ignored pause.when_covered"
    );
    assert_eq!(
        text.matches("enabled: lights.pauses").count(),
        2,
        "a choice stayed open with nothing to choose"
    );
    assert!(text.contains(r#"App.tr("While a window covers the wallpaper and it pauses:")"#));
    let hint = text.lines().find(|l| l.contains(r#"lights.covered === "follow" ?"#)).unwrap().trim();
    assert!(
        hint.starts_with(r#"text: !lights.pauses ? App.tr("Pausing while a window covers the screen is off"#)
    );
    assert!(
        hint.contains(r#"App.tr("While the covered wallpaper pauses, the lights keep changing"#),
        "{hint}"
    );
}

#[test]
fn the_lights_board_places_devices_and_zones_by_their_own_keys() {
    let files = qml_files();
    let text = |name: &str| files.iter().find(|(n, _)| n == name).map(|(_, t)| t.clone()).unwrap();
    let (board, picture) = (text("RgbBoard.qml"), text("RgbPicture.qml"));
    assert!(board.contains(r#""rgb.place." + target.key"#), "the board wrote another setting");
    assert!(board.contains(r##"d.name + "#" + i"##), "zones were not placed by their index");
    assert!(board.contains(r#"board.save("off")"#) && board.contains(r#"board.save("none")"#));
    for flow in ["right", "left", "down", "up"] {
        assert!(board.contains(&format!(r#""value": "{flow}""#)), "{flow} cannot be chosen");
    }
    assert!(picture.contains("drag.target: handle") && picture.contains("onPositionChanged"));
    assert!(text("SettingsRgb.qml").contains("RgbBoard {"));
}

#[test]
fn a_page_preview_shows_its_lights_and_gives_its_stream_back() {
    let files = qml_files();
    let text = |name: &str| files.iter().find(|(n, _)| n == name).map(|(_, t)| t.clone()).unwrap();
    let lights = text("PageLights.qml");
    assert!(lights.contains(r#""preview:page-lights-" + Date.now()"#), "the preview is not a preview output");
    assert!(lights.contains(r#"App.query("GetStrip", [trial.output], "Rgb1""#));
    assert!(
        lights.contains(r#"App.act("ReleaseStream", [output], "Daemon1")"#)
            && lights.contains("onClosed: stop()")
    );
    assert!(lights.contains("running: trial.opened"), "the strip was read while closed");
    assert!(text("ItemDrawer.qml").contains("lights.start(drawer.uid)"));
}
