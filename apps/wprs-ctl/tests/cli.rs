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
fn daemon_commands_keep_their_syntax_and_gain_json() {
    let cases = [
        ("set a.png -o DP-1 -f fit", r#"Set { path: "a.png", output: "DP-1", fit: Some(Fit) }"#),
        ("fit tile", r#"Fit { fit: Tile, output: "" }"#),
        ("background #203040 -o DP-1", r##"Background { color: "#203040", output: "DP-1" }"##),
        ("config --json", "Config { key: None, value: None, json: true }"),
        (
            "config playback.max_fps 30",
            r#"Config { key: Some("playback.max_fps"), value: Some("30"), json: false }"#,
        ),
        ("state --json", "State { json: true }"),
        ("volume 40", "Volume { percent: 40 }"),
        ("stream DP-1 320 180 --wait", r#"Stream { output: "DP-1", width: 320, height: 180, wait: true }"#),
        ("covered DP-1 true", r#"Covered { output: "DP-1", covered: true }"#),
        ("schema --json", "Schema { json: true }"),
        ("watch", "Watch { json: false }"),
        ("wait", "Wait { serial: None, timeout: 20000, json: false }"),
        ("wait --serial 7 --timeout 50 --json", "Wait { serial: Some(7), timeout: 50, json: true }"),
        ("jobs --json", "Jobs { json: true }"),
        ("job 3", "Job { id: 3, json: false }"),
        ("cancel 3", "Cancel { id: 3 }"),
        ("next -o DP-1", r#"Next(Out { output: "DP-1" })"#),
        ("prev", r#"Prev(Out { output: "" })"#),
    ];
    for (line, debug) in cases {
        assert_eq!(parsed(line), debug, "{line}");
    }
    for line in ["reload", "pause", "resume", "toggle", "mute", "unmute", "release DP-1"] {
        parsed(line);
    }
}

#[test]
fn library_and_preset_commands_parse() {
    let wait = |wait, json| format!("wait: Wait {{ wait: {wait}, json: {json} }}");
    let cases = [
        (
            "lib add /srv/pics --no-recursive --wait",
            format!(r#"Lib(Add {{ dir: "/srv/pics", no_recursive: true, {} }})"#, wait(true, false)),
        ),
        ("lib sources --json", "Lib(Sources { json: true })".into()),
        ("lib scan", format!("Lib(Scan {{ source: None, {} }})", wait(false, false))),
        ("lib unsource 1AB2C3", r#"Lib(Unsource { source: "1AB2C3" })"#.into()),
        (
            "lib import a.png b.webm --copy --wait --json",
            format!(r#"Lib(Import {{ files: ["a.png", "b.webm"], copy: true, {} }})"#, wait(true, true)),
        ),
        ("lib rm R1AB2C --delete-files", r#"Lib(Rm { items: ["R1AB2C"], delete_files: true })"#.into()),
        (
            "lib search tag:sea --offset 50 --limit 10",
            r#"Lib(Search { query: "tag:sea", page: Page { offset: 50, limit: 10 }, json: false })"#.into(),
        ),
        (
            "lib search",
            r#"Lib(Search { query: "", page: Page { offset: 0, limit: 50 }, json: false })"#.into(),
        ),
        ("lib show R1AB2C --json", r#"Lib(Show { item: "R1AB2C", json: true })"#.into()),
        ("lib title R1AB2C Harbour", r#"Lib(Title { item: "R1AB2C", title: "Harbour" })"#.into()),
        ("lib rate 4 R1AB2C X9Y8Z7", r#"Lib(Rate { rating: 4, items: ["R1AB2C", "X9Y8Z7"] })"#.into()),
        ("lib fav R1AB2C --off", r#"Lib(Fav { items: ["R1AB2C"], off: true })"#.into()),
        (
            "lib confirm photosensitive R1AB2C",
            r#"Lib(Confirm { what: "photosensitive", items: ["R1AB2C"] })"#.into(),
        ),
        (
            "lib tag R1AB2C --add sea --add blue --remove old",
            r#"Lib(Tag { items: ["R1AB2C"], add: ["sea", "blue"], remove: ["old"] })"#.into(),
        ),
        ("lib tags", "Lib(Tags { json: false })".into()),
        ("lib retag Sea sea", r#"Lib(Retag { from: "Sea", to: "sea" })"#.into()),
        ("lib thumb R1AB2C --size 1024", r#"Lib(Thumb { item: "R1AB2C", size: "1024" })"#.into()),
        ("lib thumb R1AB2C", r#"Lib(Thumb { item: "R1AB2C", size: "512" })"#.into()),
        (
            "lib apply R1AB2C -o DP-1 -f center -p dark",
            r#"Lib(Apply { item: "R1AB2C", output: "DP-1", fit: Some(Center), preset: "dark" })"#.into(),
        ),
        ("preset ls", "Preset(Ls { item: None, json: false })".into()),
        ("preset ls R1AB2C --json", r#"Preset(Ls { item: Some("R1AB2C"), json: true })"#.into()),
        (
            "preset save - dark fit=fit background=#000000",
            r##"Preset(Save { item: "-", name: "dark", values: ["fit=fit", "background=#000000"] })"##.into(),
        ),
        ("preset rm -:dark", r#"Preset(Rm { preset: "-:dark" })"#.into()),
    ];
    for (line, debug) in cases {
        assert_eq!(parsed(line), debug, "{line}");
    }
}

#[test]
fn playlist_and_profile_commands_parse() {
    let cases = [
        ("playlist ls", "Playlist(Ls { json: false })"),
        (
            "playlist show Sea --offset 5 --limit 5 --json",
            r#"Playlist(Show { playlist: "Sea", page: Page { offset: 5, limit: 5 }, json: true })"#,
        ),
        ("playlist new Sea --query tag:sea", r#"Playlist(New { name: "Sea", query: "tag:sea" })"#),
        ("playlist new Mine", r#"Playlist(New { name: "Mine", query: "" })"#),
        ("playlist rename Sea Ocean", r#"Playlist(Rename { playlist: "Sea", name: "Ocean" })"#),
        ("playlist query Sea tag:ocean", r#"Playlist(Query { playlist: "Sea", query: "tag:ocean" })"#),
        ("playlist rm Sea", r#"Playlist(Rm { playlist: "Sea" })"#),
        (
            "playlist add Mine R1AB2C X9Y8Z7",
            r#"Playlist(Add { playlist: "Mine", items: ["R1AB2C", "X9Y8Z7"] })"#,
        ),
        ("playlist remove Mine R1AB2C", r#"Playlist(Remove { playlist: "Mine", items: ["R1AB2C"] })"#),
        ("playlist remove-at Mine 0 3", r#"Playlist(RemoveAt { playlist: "Mine", indexes: [0, 3] })"#),
        ("playlist move Mine 4 0", r#"Playlist(Move { playlist: "Mine", from: 4, to: 0 })"#),
        ("playlist rule Mine", r#"Playlist(Rule { playlist: "Mine", key: None, value: None, json: false })"#),
        (
            "playlist rule Mine interval 1h",
            r#"Playlist(Rule { playlist: "Mine", key: Some("interval"), value: Some("1h"), json: false })"#,
        ),
        (
            "playlist start Mine -o DP-1",
            r#"Playlist(Start { playlist: "Mine", out: Out { output: "DP-1" } })"#,
        ),
        ("playlist stop", r#"Playlist(Stop(Out { output: "" }))"#),
        (
            "playlist history -o DP-1 --limit 5",
            r#"Playlist(History { output: "DP-1", limit: 5, json: false })"#,
        ),
        ("profile ls --json", "Profile(Ls { json: true })"),
        ("profile show Desk", r#"Profile(Show { profile: "Desk", json: false })"#),
        ("profile save Desk --no-options", r#"Profile(Save { name: "Desk", no_options: true })"#),
        ("profile use Desk", r#"Profile(Use { profile: "Desk" })"#),
        ("profile off", "Profile(Off)"),
        ("profile rename Desk Office", r#"Profile(Rename { profile: "Desk", name: "Office" })"#),
        ("profile rm Desk", r#"Profile(Rm { profile: "Desk" })"#),
        (
            "profile auto Desk on_battery true",
            r#"Profile(Auto { profile: "Desk", key: Some("on_battery"), value: Some("true"), json: false })"#,
        ),
    ];
    for (line, debug) in cases {
        assert_eq!(parsed(line), debug, "{line}");
    }
}

#[test]
fn provider_and_package_commands_parse() {
    let wait = r#"wait: Wait { wait: true, json: true }"#;
    let cases = [
        ("provider ls", "Provider(Ls { json: false })".into()),
        ("provider add photos immich server=https://photos.example.org album=a1 --allow-plain-http --secret-stdin", r#"Provider(Add { id: "photos", kind: "immich", options: ["server=https://photos.example.org", "album=a1"], allow_plain_http: true, no_secret: false, secret_stdin: true })"#.to_string()),
        ("provider add daily bing --no-secret", r#"Provider(Add { id: "daily", kind: "bing", options: [], allow_plain_http: false, no_secret: true, secret_stdin: false })"#.into()),
        ("provider set photos album b2", r#"Provider(Set { id: "photos", key: "album", value: "b2" })"#.into()),
        ("provider secret photos", r#"Provider(Secret { id: "photos", secret_stdin: false })"#.into()),
        ("provider forget-secret photos", r#"Provider(ForgetSecret { id: "photos" })"#.into()),
        ("provider sync --wait --json", format!("Provider(Sync {{ id: None, {wait} }})")),
        ("provider rm photos --purge", r#"Provider(Rm { id: "photos", purge: true })"#.into()),
        ("remote ls", "Remote(Ls { json: false })".into()),
        ("remote add https://example.org/repo?fingerprint=ab --wait --json", format!(r#"Remote(Add {{ url: "https://example.org/repo?fingerprint=ab", {wait} }})"#)),
        ("remote refresh 0123abcd", r#"Remote(Refresh { repo: Some("0123abcd"), wait: Wait { wait: false, json: false } })"#.into()),
        ("remote rm 0123abcd --uninstall", r#"Remote(Rm { repo: "0123abcd", uninstall: true })"#.into()),
        ("pkg search dawn --limit 5", r#"Pkg(Search { query: "dawn", page: Page { offset: 0, limit: 5 }, json: false })"#.into()),
        ("pkg preview org.example.dawn", r#"Pkg(Preview { spec: "org.example.dawn" })"#.into()),
        ("pkg install 0123abcd/org.example.dawn@1.0.0 --allow-stale --downgrade --wait --json", format!(r#"Pkg(Install {{ spec: "0123abcd/org.example.dawn@1.0.0", allow_stale: true, downgrade: true, {wait} }})"#)),
        ("pkg upgrade", r#"Pkg(Upgrade { allow_stale: false, wait: Wait { wait: false, json: false } })"#.into()),
        ("pkg uninstall org.example.dawn --version 1.0.0", r#"Pkg(Uninstall { id: "org.example.dawn", version: "1.0.0" })"#.into()),
        ("pkg ls --json", "Pkg(Ls { json: true })".into()),
    ];
    for (line, debug) in cases {
        assert_eq!(parsed(line), debug, "{line}");
    }
}

#[test]
fn operator_commands_parse() {
    let cases = [
        (
            "pack build src -o out.wpak --json",
            r#"Pack(Build { dir: "src", out: Some("out.wpak"), json: true })"#,
        ),
        ("pack sign src --key author.key", r#"Pack(Sign { dir: "src", key: "author.key" })"#),
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
