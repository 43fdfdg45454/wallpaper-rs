use super::*;

#[test]
fn packages_are_read_again_after_an_upgrade_and_titles_follow_the_language() {
    let dir = TempDir::new().unwrap();
    let (db, root) = (dir.path().join("library.db"), dir.path().join("pictures"));
    let titles = BTreeMap::from([("en".to_string(), "Storm".to_string()), ("es".into(), "Tormenta".into())]);
    build(&root, "storm", Some(Rating::General), false, Text::Localized(titles));
    let mut library = Library::open(&db).unwrap();
    let source = library.add_folder(&root, true).unwrap();
    library.scan(&source, NOW).unwrap();
    assert_eq!(library.reread().unwrap(), 0);
    let mut items = library.search("", NOW, 0, 10).unwrap();
    let uid = items[0].uid.clone();
    let title = |library: &Library, languages: &[&str], items: &mut Vec<wprs_library::Item>| {
        let languages: Vec<String> = languages.iter().map(|l| l.to_string()).collect();
        library.localize(items, &languages).unwrap();
        items[0].title.clone()
    };
    assert_eq!(
        (title(&library, &["es_ES"], &mut items), title(&library, &["fr"], &mut items)),
        ("Tormenta".into(), "Storm".into())
    );
    let older = "UPDATE items SET pkg_read = NULL, facets = NULL, content = NULL, pkg_id = NULL, title = 'storm', \
        error = 'this package needs a newer version of wallpaper-rs: it uses the feature meshes'";
    Connection::open(&db).unwrap().execute(older, []).unwrap();
    assert_eq!(library.reread().unwrap(), 1);
    let item = library.item(&uid).unwrap();
    let package = item.package.unwrap();
    assert_eq!(
        (item.meta.error, package.content.as_deref(), package.facets),
        (None, Some("general"), ["android", "desktop", "landscape", "localized"].map(String::from).to_vec())
    );
    assert!(["Storm", "Tormenta"].contains(&item.title.as_str()), "{}", item.title);
    library.set_title(&uid, "Mine").unwrap();
    let mut items = vec![library.item(&uid).unwrap()];
    assert_eq!(title(&library, &["es"], &mut items), "Mine");
}

#[test]
fn packages_this_device_cannot_show_keep_their_details_and_say_why() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().join("pictures");
    build(&root, "phone", Some(Rating::General), false, "Phone".into());
    build(&root, "mirror", Some(Rating::General), false, "Mirror".into());
    let edit = |name: &str, change: fn(&mut serde_json::Value)| {
        let source = dir.path().join(name);
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(source.join("manifest.json")).unwrap()).unwrap();
        change(&mut manifest);
        fs::write(source.join("manifest.json"), manifest.to_string()).unwrap();
        wprs_format::build(&source, root.join(format!("{name}.wpak")), &Limits::default()).unwrap();
    };
    edit("phone", |m| {
        (m["platforms"], m["requires"]) = (json!(["android"]), json!({"features": ["platforms"]}))
    });
    edit("mirror", |m| {
        (m["display"], m["requires"]) =
            (json!({"flip": "horizontal"}), json!({"features": ["display2", "flip"]}))
    });
    let mut library = Library::open(dir.path().join("library.db")).unwrap();
    let source = library.add_folder(&root, true).unwrap();
    library.scan(&source, NOW).unwrap();
    let found = |title: &str| library.search(title, NOW, 0, 1).unwrap().remove(0);
    let (phone, mirror) = (found("Phone"), found("Mirror"));
    assert_eq!(phone.meta.error.as_deref(), Some("this package is made for Android only"));
    assert_eq!(phone.package.unwrap().facets, ["android", "landscape"]);
    let error = mirror.meta.error.unwrap();
    assert!(error.contains("needs a newer version") && error.contains("flip (shows its picture"), "{error}");
    assert_eq!(mirror.package.unwrap().id, "org.example.mirror");
}

#[test]
fn packages_refused_by_the_strike_book_are_read_again_when_effects_are_retried() {
    let dir = TempDir::new().unwrap();
    let (db, root) = (dir.path().join("library.db"), dir.path().join("pictures"));
    package(&root, "storm", None, false);
    package(&root, "calm", None, false);
    let mut library = Library::open(&db).unwrap();
    let source = library.add_folder(&root, true).unwrap();
    library.scan(&source, NOW).unwrap();
    let blocked = "invalid scene: /effects/s: its shader is not checked again: it stopped the graphics card";
    let other = "this package needs a newer version of wallpaper-rs";
    let refuse = "UPDATE items SET error = ?1, pkg_id = NULL, title = 'storm' WHERE path LIKE '%storm%'";
    let connection = Connection::open(&db).unwrap();
    connection.execute(refuse, [blocked]).unwrap();
    connection.execute("UPDATE items SET error = ?1 WHERE path LIKE '%calm%'", [other]).unwrap();
    let storm = library.search("storm", NOW, 0, 10).unwrap().remove(0).uid;
    assert_eq!(library.recheck("not checked again").unwrap(), std::slice::from_ref(&storm));
    let item = library.item(&storm).unwrap();
    assert_eq!((item.meta.error, item.package.map(|p| p.id)), (None, Some("org.example.storm".into())));
    assert!(library.recheck("not checked again").unwrap().is_empty());
    let calm = library.search("calm", NOW, 0, 10).unwrap().remove(0);
    assert_eq!(calm.meta.error.as_deref(), Some(other), "only refusals from the book are read again");
}
