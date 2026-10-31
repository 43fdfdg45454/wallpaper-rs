use clap::Parser;
use wprs_ctl::Cli;

fn parse(args: &[&str]) -> Result<String, clap::Error> {
    Cli::try_parse_from(["wprs-ctl"].iter().chain(args)).map(|cli| format!("{:?}", cli.command))
}

fn parsed(line: &str) -> String {
    let args: Vec<&str> = line.split(' ').collect();
    parse(&args).unwrap_or_else(|e| panic!("{line}: {e}"))
}

#[test]
fn operator_commands_parse() {
    let cases = [
        (
            "pack build src -o out.wpak --json",
            r#"Pack(Build { dir: "src", out: Some("out.wpak"), json: true })"#,
        ),
        ("pack sign src --key author.key", r#"Pack(Sign { dir: "src", key: "author.key" })"#),
        (
            "pack rotate-key old.key new.pub --author u7",
            r#"Pack(RotateKey { old: "old.key", new: "new.pub", author: "u7" })"#,
        ),
        (
            "moderate --server https://example.org hide org.example.dawn --reason spam-reason",
            r#"Moderate(Moderate { server: "https://example.org", action: Hide { id: "org.example.dawn", reason: "spam-reason" } })"#,
        ),
        ("pack check src --json", r#"Pack(Check { path: "src", json: true })"#),
        (
            "pack upgrade src --rating mature --reason gore --photosensitive true --derived-from Ana",
            r#"Pack(Upgrade { dir: "src", rating: "mature", reasons: ["gore"], photosensitive: true, derived_from: ["Ana"] })"#,
        ),
        (
            "pack validate a.wpak b.wpak --key author.pub",
            r#"Pack(Validate { files: ["a.wpak", "b.wpak"], keys: ["author.pub"], json: false })"#,
        ),
        ("repo keygen root --no-password", r#"Repo(Keygen { name: "root", no_password: true })"#),
        (
            "repo sign-root site --key root.key",
            r#"Repo(SignRoot { dir: "site", key: "root.key", json: false })"#,
        ),
        ("repo status site --json", r#"Repo(Status { dir: "site", json: true })"#),
        (
            "repo add site a.wpak --author studio",
            r#"Repo(Add { dir: "site", files: ["a.wpak"], author: Some("studio"), json: false })"#,
        ),
        (
            "repo revoke site org.example.dawn --version 1.0.0 --reason broken",
            r#"Repo(Revoke { dir: "site", id: "org.example.dawn", version: Some("1.0.0"), reason: "broken", message: "" })"#,
        ),
        (
            "repo revoke-key site ab12 --reason malware",
            r#"Repo(RevokeKey { dir: "site", fingerprint: "ab12", reason: "malware" })"#,
        ),
        (
            "repo publish site --key online.key",
            r#"Repo(Publish(Publish { dir: "site", key: "online.key", days: 7, json: false }))"#,
        ),
        (
            "repo refresh site --key online.key --days 3 --json",
            r#"Repo(Refresh(Publish { dir: "site", key: "online.key", days: 3, json: true }))"#,
        ),
        (
            "repo verify https://example.org/repo",
            r#"Repo(Verify { url: "https://example.org/repo", json: false })"#,
        ),
    ];
    for (line, debug) in cases {
        assert_eq!(parsed(line), debug, "{line}");
    }
    let init = parsed(
        "repo init site --name Example --root-key r.pub --online-key o.pub --author studio=a.pub --require-author-signature --mirror https://mirror.example.org",
    );
    assert!(init.contains(r#"authors: ["studio=a.pub"], require_author_signature: Some(true)"#), "{init}");
    let open = parsed("repo init site --name Example --root-key r.pub --online-key o.pub --online-authors");
    assert!(open.contains("online_authors: Some(true)"), "{open}");
    assert!(
        init.contains(r#"root_days: None"#) && init.contains(r#"mirrors: ["https://mirror.example.org"]"#),
        "{init}"
    );
    let rotate = parsed(
        "repo rotate-root site --remove-author studio --require-author-signature false --root-days 400",
    );
    assert!(
        rotate.contains(r#"removed: ["studio"]"#) && rotate.contains("require_author_signature: Some(false)")
    );
    assert!(rotate.contains("root_days: Some(400)"), "{rotate}");
}

#[test]
fn usage_errors_exit_with_2() {
    let refused = [
        &["lib", "tag", "R1AB2C"][..],
        &["lib", "rate", "6", "R1AB2C"],
        &["lib", "rm"],
        &["lib", "thumb", "R1AB2C", "--size", "300"],
        &["playlist", "history"],
        &["repo", "revoke", "site", "x", "--reason", "boredom"],
        &["repo", "init", "site", "--name", "Example", "--online-key", "o.pub"],
        &["pkg", "install"],
        &["volume", "101"],
    ];
    for args in refused {
        assert_eq!(parse(args).unwrap_err().exit_code(), 2, "{args:?}");
    }
}

#[test]
fn settings_take_negative_numbers() {
    let cases = [
        ("prop set x -0.5 -o DP-1", r#"Prop(Set { id: "x", value: Some("-0.5"), output: "DP-1" })"#),
        ("prop set y -1", r#"Prop(Set { id: "y", value: Some("-1"), output: "" })"#),
    ];
    for (line, debug) in cases {
        assert_eq!(parsed(line), debug, "{line}");
    }
}
