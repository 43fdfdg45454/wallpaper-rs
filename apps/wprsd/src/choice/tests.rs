use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn a_profile_chosen_by_hand_holds_until_its_conditions_change() {
    let dir = tempfile::tempdir().unwrap();
    let (docked, away) = ((BTreeSet::from(["DP-1".to_owned()]), false), (BTreeSet::new(), false));
    assert!(!holds(dir.path(), Some("P"), &docked));
    keep(dir.path(), Some((Some("P".into()), docked.clone()))).unwrap();
    let mode = std::fs::metadata(dir.path().join(HOLD)).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    assert!(holds(dir.path(), Some("P"), &docked) && !holds(dir.path(), Some("P"), &away));
    assert!(holds(dir.path(), Some("P"), &docked) && !holds(dir.path(), None, &docked));
    assert!(!holds(dir.path(), Some("P"), &docked));
    keep(dir.path(), Some((Some("P".into()), docked.clone()))).unwrap();
    assert!(!holds(dir.path(), Some("P"), &(docked.0.clone(), true)));
    assert!(!holds(dir.path(), Some("P"), &docked));
    keep(dir.path(), None).unwrap();
}

#[test]
fn a_failed_hold_reports_no_path() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(HOLD)).unwrap();
    let docked = (BTreeSet::from(["DP-1".to_owned()]), false);
    let failed = keep(dir.path(), Some((Some("P".into()), docked))).unwrap_err();
    let shown = failed.to_string();
    assert!(!shown.contains(&*dir.path().to_string_lossy()) && !shown.contains(".tmp-"), "{shown}");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn an_automatic_choice_waits_for_a_choice_by_hand_and_keeps_it() {
    let dir = tempfile::tempdir().unwrap();
    let docked = (BTreeSet::from(["DP-1".to_owned()]), false);
    let auto = serde_json::json!({"outputs": ["DP-1"]});
    let profile = |uid: &str, auto| wprs_library::Profile {
        uid: uid.into(),
        name: uid.into(),
        outputs: serde_json::json!({}),
        options: serde_json::json!({}),
        auto,
        priority: 0,
    };
    let profiles = vec![profile("DOCK", auto), profile("HAND", serde_json::json!({}))];
    assert_eq!(automatic(dir.path(), Some("HAND"), &docked, &profiles).as_deref(), Some("DOCK"));
    let (path, hold, (started, applying)) =
        (dir.path().to_owned(), docked.clone(), std::sync::mpsc::channel());
    let by_hand = std::thread::spawn(move || {
        let applied = |_: &str| {
            started.send(()).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(200));
            keep(&path, Some((Some("HAND".into()), hold))).map_err(|e| crate::error::failed(format!("{e:?}")))
        };
        choose(|| Ok(Some("HAND".into())), applied)
    });
    applying.recv().unwrap();
    let mut chosen = None;
    let decide = || Ok(automatic(dir.path(), Some("HAND"), &docked, &profiles));
    let remember = |p: &str| {
        chosen = Some(p.to_owned());
        Ok(())
    };
    choose(decide, remember).unwrap();
    by_hand.join().unwrap().unwrap();
    assert_eq!(chosen, None, "an automatic choice replaced a choice made by hand meanwhile");
}
