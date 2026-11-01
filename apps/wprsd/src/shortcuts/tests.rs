use super::launch::{entry, valid_id, words};
use super::{Target, target};
use serde_json::json;

#[test]
fn shortcut_values_name_https_addresses_absolute_files_and_folders_or_application_ids() {
    let cases = [
        (json!({ "uri": "https://example.org/a" }), Some(Target::Uri("https://example.org/a".into()))),
        (json!({ "file": "/srv/notes.txt" }), Some(Target::File("/srv/notes.txt".into()))),
        (json!({ "folder": "/srv/photos" }), Some(Target::Folder("/srv/photos".into()))),
        (json!({ "app": "org.kde.dolphin" }), Some(Target::App("org.kde.dolphin".into()))),
        (json!({ "uri": "http://example.org" }), None),
        (json!({ "uri": "file:///etc/passwd" }), None),
        (json!({ "file": "notes.txt" }), None),
        (json!({ "folder": "/srv/../etc" }), None),
        (json!({ "app": "../evil" }), None),
        (json!({ "app": "a b" }), None),
        (json!({ "app": "x", "uri": "https://example.org" }), None),
        (json!("https://example.org"), None),
        (json!(null), None),
    ];
    for (value, want) in cases {
        assert_eq!(target(&value), want, "{value}");
    }
    assert!(
        valid_id("org.gnome.Nautilus") && !valid_id("") && !valid_id("-x") && !valid_id(&"a".repeat(256))
    );
}

#[test]
fn desktop_entries_give_their_program_without_field_codes_or_a_shell() {
    let text = "[Desktop Entry]\nType=Application\nExec=\"/opt/My App/run\" --new-window %U\nName=App\n[Desktop Action x]\nExec=other\n";
    let found = entry(text);
    assert_eq!((found["Type"], found["Name"]), ("Application", "App"));
    assert_eq!(words(found["Exec"]), ["/opt/My App/run", "--new-window"]);
    assert_eq!(words("sh -c \"echo \\\"hi\\\"\" 100%%"), ["sh", "-c", "echo \"hi\"", "100%"]);
    assert_eq!(words("app %x"), Vec::<String>::new(), "an unknown field code makes the entry unusable");
    assert_eq!(words("a  b\t\"\""), ["a", "b", ""]);
}
