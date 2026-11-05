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
