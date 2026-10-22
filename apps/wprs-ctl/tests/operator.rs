use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output, Stdio};

const PASSWORD: &str = "correct horse battery";

fn run(dir: &Path, args: &[&str], password: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_wprs-ctl"));
    command.args(args).current_dir(dir).stdin(Stdio::null()).env_remove("WPRS_KEY_PASSWORD");
    command.env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={}/no-bus", dir.display()));
    if let Some(password) = password {
        command.env("WPRS_KEY_PASSWORD", password);
    }
    command.output().unwrap()
}

fn ok(dir: &Path, args: &[&str]) -> String {
    let out = run(dir, args, Some(PASSWORD));
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap().trim_end().into()
}

fn fails(dir: &Path, args: &[&str], code: i32, password: Option<&str>) -> String {
    let out = run(dir, args, password);
    assert_eq!(out.status.code(), Some(code), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    String::from_utf8(out.stderr).unwrap()
}

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn source(dir: &Path, version: &str) {
    let manifest = format!(
        r#"{{"schema": "wallpaper-rs.wpak/1", "id": "org.example.dawn", "version": "{version}",
        "title": "Sky at dawn", "license": "CC0-1.0", "kind": "image", "entry": "image.png"}}"#
    );
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(dir.join("src/manifest.json"), manifest).unwrap();
    fs::write(dir.join("src/image.png"), format!("pixels {version}")).unwrap();
}

#[test]
fn keys_are_created_once_with_a_private_secret_file() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let fingerprint = ok(dir, &["repo", "keygen", "root"]);
    assert_eq!(fingerprint.len(), 64);
    let mode = |name: &str| fs::metadata(dir.join(name)).unwrap().permissions().mode() & 0o777;
    assert_eq!((mode("root.key"), mode("root.pub") & 0o600), (0o600, 0o600));
    assert!(fs::read_to_string(dir.join("root.key")).unwrap().contains("secret key"));
    let refused = fails(dir, &["repo", "keygen", "root", "--no-password"], 1, None);
    assert!(refused.contains("root.key or root.pub already exists"), "{refused}");
    let asked = fails(dir, &["repo", "keygen", "other"], 1, None);
    assert!(asked.contains("WPRS_KEY_PASSWORD"), "{asked}");
    assert!(!dir.join("other.key").exists());
    assert!(fails(dir, &["repo", "keygen", "other"], 1, Some("")).contains("--no-password"));
}

#[test]
fn packages_are_built_signed_and_validated() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let author = ok(dir, &["repo", "keygen", "author"]);
    let stranger = ok(dir, &["repo", "keygen", "stranger", "--no-password"]);
    source(dir, "1.0.0");
    assert!(fails(dir, &["pack", "sign", "src", "--key", "author.key"], 1, Some("wrong")).contains("Error"));
    assert_eq!(ok(dir, &["pack", "sign", "src", "--key", "author.key"]), author);
    let built = ok(dir, &["pack", "build", "src"]);
    let [sha256, size, path] = built.split(' ').collect::<Vec<_>>()[..] else { panic!("{built}") };
    assert_eq!(
        (path, size.parse().unwrap()),
        ("org.example.dawn-1.0.0.wpak", fs::metadata(dir.join(path)).unwrap().len())
    );
    let copy = json(&ok(dir, &["pack", "build", "src", "-o", "copy.wpak", "--json"]));
    assert_eq!((copy["path"].as_str(), copy["sha256"].as_str()), (Some("copy.wpak"), Some(sha256)));
    let signed = ok(dir, &["pack", "validate", path, "--key", "author.pub"]);
    assert_eq!(signed, format!("ok image org.example.dawn 1.0.0 sha256={sha256} signer={author}"));
    assert!(ok(dir, &["pack", "validate", path]).ends_with("signer=-"));
    fs::write(dir.join("broken.wpak"), "not a zip").unwrap();
    let out = run(dir, &["pack", "validate", path, "broken.wpak", "--key", "stranger.pub", "--json"], None);
    assert_eq!(out.status.code(), Some(1));
    let reports: Vec<Value> = String::from_utf8(out.stdout).unwrap().lines().map(json).collect();
    assert_eq!(reports.iter().map(|r| r["valid"].as_bool()).collect::<Vec<_>>(), [Some(false), Some(false)]);
    assert_eq!(reports[0]["manifest"], Value::Null);
    assert!(reports[0]["error"].as_str().unwrap().contains("signature"), "{}", reports[0]);
    assert!(String::from_utf8(out.stderr).unwrap().contains("2 of 2 packages are invalid"));
    assert_eq!(stranger.len(), 64);
}

#[test]
fn a_repository_is_initialized_published_rotated_and_verified() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let root = ok(dir, &["repo", "keygen", "root"]);
    for name in ["online", "author", "next"] {
        ok(dir, &["repo", "keygen", name, "--no-password"]);
    }
    source(dir, "1.0.0");
    ok(dir, &["pack", "sign", "src", "--key", "author.key"]);
    ok(dir, &["pack", "build", "src", "-o", "dawn.wpak"]);
    let init =
        ["repo", "init", "site", "--name", "Example", "--root-key", "root.pub", "--online-key", "online.pub"];
    let created = json(&ok(dir, &[&init[..], &["--author", "studio=author.pub", "--json"]].concat()));
    assert_eq!((created["version"].as_u64(), created["complete"].as_bool()), (Some(1), Some(false)));
    assert!(fails(dir, &["repo", "publish", "site", "--key", "online.key"], 1, None).contains("root"));
    let signed = ok(dir, &["repo", "sign-root", "site", "--key", "root.key"]);
    assert!(signed.contains(&format!("signers = {root}")) && signed.contains("complete = true"), "{signed}");
    let anchor = ok(dir, &["repo", "status", "site"]).lines().last().unwrap().to_string();
    assert!(anchor.starts_with(&format!("anchor = ?fingerprint={root}&repo_id=")), "{anchor}");
    let remote = fails(dir, &["repo", "add", "site", "https://example.org/dawn.wpak"], 2, None);
    assert!(remote.contains("use wprs-ctl remote add"), "{remote}");
    let added = ok(dir, &["repo", "add", "site", "dawn.wpak", "--author", "studio"]);
    assert!(
        added.starts_with("org.example.dawn 1.0.0 sha256=") && added.ends_with("author=studio"),
        "{added}"
    );
    assert!(
        ok(dir, &["repo", "publish", "site", "--key", "online.key"]).starts_with("index 1 published with 1")
    );
    let verified = ok(dir, &["repo", "verify", "site"]);
    assert!(verified.contains("name = Example") && verified.contains("packages = 1"), "{verified}");
    let url = format!("file://{}/site/{}", dir.display(), &anchor["anchor = ".len()..]);
    assert_eq!(json(&ok(dir, &["repo", "verify", &url, "--json"]))["name"], "Example");
    ok(dir, &["repo", "revoke", "site", "org.example.dawn", "--version", "1.0.0", "--reason", "broken"]);
    let refreshed =
        json(&ok(dir, &["repo", "refresh", "site", "--key", "online.key", "--days", "3", "--json"]));
    assert_eq!(refreshed["version"], 2);
    let rotate =
        ["repo", "rotate-root", "site", "--root-key", "next.pub", "--remove-author", "studio", "--json"];
    assert_eq!(json(&ok(dir, &rotate))["complete"], false);
    assert!(ok(dir, &["repo", "sign-root", "site", "--key", "next.key"]).contains("complete = false"));
    assert!(ok(dir, &["repo", "sign-root", "site", "--key", "root.key"]).contains("complete = true"));
    ok(dir, &["repo", "publish", "site", "--key", "online.key"]);
    let info = json(&ok(dir, &["repo", "verify", "site", "--json"]));
    assert_eq!((info["root_version"].as_u64(), info["entry_version"].as_u64()), (Some(2), Some(3)));
    assert!(fails(dir, &["repo", "verify", "missing"], 1, None).contains("Error"));
}
