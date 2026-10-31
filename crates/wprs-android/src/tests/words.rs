use crate::{Error, said};
use serde_json::json;
use std::fs;
use std::path::Path;

fn ids(dir: &Path, found: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() && !path.ends_with("tests") {
            ids(&path, found);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = fs::read_to_string(&path).unwrap().replace(char::is_whitespace, "");
            let calls = text.split("said(\"android.").skip(1);
            found.extend(calls.filter_map(|rest| Some(format!("android.{}", rest.split_once('"')?.0))));
        }
    }
}

#[test]
fn every_message_of_the_phone_has_a_string_in_each_language() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = vec!["android.unreadable".to_owned(), "format.newer".into(), "format.elsewhere".into()];
    ids(&src, &mut found);
    assert!(found.len() > 50, "{found:?}");
    let res = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../android/app/src/main/res");
    for folder in ["values", "values-es"] {
        let strings = fs::read_to_string(res.join(folder).join("errors.xml")).unwrap();
        let missing: Vec<_> = (found.iter())
            .filter(|id| !strings.contains(&format!("name=\"error_{}\"", id.replace('.', "_"))))
            .collect();
        assert!(missing.is_empty(), "{folder} lacks {missing:?}");
    }
}

#[test]
fn errors_carry_their_message_id_and_arguments() {
    let error = said("android.no_preset", "no preset %1", &[&"Calm"]);
    assert_eq!(
        error.reply(),
        json!({ "error": "no preset Calm", "id": "android.no_preset", "args": ["Calm"] })
    );
    let unreadable = Error::Unreadable("notes.txt".into()).reply();
    assert_eq!(
        (&unreadable["id"], &unreadable["args"]),
        (&json!("android.unreadable"), &json!(["notes.txt"]))
    );
    let other = Error::Setting("x".into()).reply();
    assert_eq!((&other["error"], &other["id"]), (&json!("x"), &json!("")));
}
