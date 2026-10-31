use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn daemon_errors_map_to_messages_and_exit_codes() {
    let bus = Bus::start().await;
    let (code, _, err) = bus.ctl(&["lib", "show", "nosuch"]);
    assert_eq!((code, err.as_str()), (1, "Error: no item matches \"nosuch\"\n"));
    let (code, _, err) = bus.ctl(&["lib", "show", "https://example.org/a.png"]);
    assert_eq!((code, err.as_str()), (2, "Error: https://example.org/a.png is not a local file\n"));
    assert!(!bus.log.has("Resolve https"));
    let (code, _, err) = bus.ctl(&["pkg", "install", "org.example.dawn"]);
    assert_eq!(
        (code, err.as_str()),
        (1, "Error: repository 01234567 is stale: its signed index has expired\n")
    );
    assert!(bus.log.has("Browse org.example.dawn 0 500") && !bus.log.has("Install org.example.dawn true"));
    assert_eq!(
        bus.ctl(&["pkg", "install", "org.example.dawn", "--allow-stale"]),
        (0, "5\n".into(), String::new())
    );
    let (code, _, err) = bus.ctl(&["profile", "ls"]);
    let older = "Error: wprsd is older than this client; install matching versions of wallpaper-rs\n";
    assert_eq!((code, err.as_str()), (1, older));
}

#[tokio::test(flavor = "multi_thread")]
async fn provider_secrets_come_from_stdin_and_never_from_arguments() {
    let bus = Bus::start().await;
    let add =
        ["provider", "add", "photos", "immich", "server=https://photos.example.org", "--allow-plain-http"];
    let (code, out, err) = bus.fed(&[&add[..], &["--secret-stdin"]].concat(), "s3cret\nignored\n");
    assert_eq!((code, out.as_str()), (0, format!("{}\n", UIDS[0]).as_str()), "{err}");
    let options = r#"{"allow_plain_http": "true", "server": "https://photos.example.org"}"#;
    assert!(bus.log.has(&format!("Add photos immich {options} [s3cret]")));
    let (code, _, err) = bus.fed(&add, "");
    assert!(code == 2 && err.contains("immich needs its api_key; use --no-secret"), "{err}");
    assert_eq!(bus.ctl(&[&add[..], &["--no-secret"]].concat()).0, 0);
    assert!(bus.log.has(&format!("Add photos immich {options} []")));
    let (code, _, err) = bus.fed(&[&add[..], &["api_key=s3cret"]].concat(), "s3cret\n");
    assert!(code == 2 && err.contains("api_key is a secret"), "{err}");
    assert!(!bus.log.has("Add photos immich {\"allow_plain_http\": \"true\", \"api_key\""));
}

#[tokio::test(flavor = "multi_thread")]
async fn playlist_rows_far_apart_are_read_before_removal() {
    let bus = Bus::start().await;
    let (code, _, err) = bus.ctl(&["playlist", "remove-at", "Mine", "551", "3", "550"]);
    assert_eq!(code, 0, "{err}");
    assert!(bus.log.has("Get Mine 3 500") && bus.log.has("Get Mine 550 500") && !bus.log.has("Get Mine 551"));
    let expected = format!("{:?}", [UIDS[1], UIDS[3], UIDS[0]]);
    assert!(bus.log.has(&format!("RemoveEntries Mine [551, 3, 550] {expected}")));
    let (code, _, err) = bus.ctl(&["playlist", "remove-at", "Mine", "600"]);
    assert_eq!((code, err.as_str()), (2, "Error: the playlist has no entry 600\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_upgrade_tries_every_repository_before_failing_on_stale_ones() {
    let bus = Bus::start().await;
    let (code, out, _) = bus.ctl(&["pkg", "upgrade"]);
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(bus.log.has("Install 0123456789abcdef/org.example.dawn false false"));
    assert!(bus.log.has("Install fedcba9876543210/org.example.dusk false false"));
    let (code, out, _) = bus.ctl(&["pkg", "upgrade", "--allow-stale"]);
    assert_eq!((code, out.as_str()), (0, "5\n5\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn arguments_refused_after_connecting_are_usage_errors() {
    let bus = Bus::start().await;
    let (code, _, err) = bus.ctl(&["config", "playback.max_fps", "999"]);
    assert_eq!((code, err.as_str()), (2, "Error: invalid value \"999\" for playback.max_fps\n"));
    assert!(bus.log.has("SetOption playback.max_fps 999"));
    let (code, _, err) = bus.ctl(&["provider", "add", "photos", "nosuch"]);
    assert!(code == 2 && err.contains("unknown provider kind nosuch; the kinds are immich"), "{err}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_preset_of_a_file_with_colons_in_its_path_is_found() {
    let bus = Bus::start().await;
    let dir = tempfile::tempdir().unwrap();
    let picture = dir.path().join("a:b.png");
    std::fs::write(&picture, b"").unwrap();
    let (code, _, err) = bus.ctl(&["preset", "rm", &format!("{}:Dim: late", picture.display())]);
    assert_eq!(code, 0, "{err}");
    assert!(bus.log.has(&format!("Resolve {}", picture.display())));
    assert!(bus.log.has(&format!("ListPresets {}", UIDS[2])));
    assert!(bus.log.has(&format!("DeletePreset {}", UIDS[3])));
    assert_eq!(bus.ctl(&["preset", "rm", &format!("{}:late", picture.display())]).0, 0);
    assert!(bus.log.has(&format!("DeletePreset {}", UIDS[4])));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_grant_names_one_package_and_the_status_shows_what_each_screen_may_use() {
    let bus = Bus::start().await;
    let grant =
        |output: &[&str]| bus.ctl(&[&["input", "grant", "org.example.sea", "audio"][..], output].concat());
    let (code, _, err) = grant(&[]);
    assert!(code == 2 && err.contains("choose one with --output (DP-1, DP-2)"), "{err}");
    assert!(!bus.log.has("GrantPackage"));
    assert_eq!(grant(&["-o", "DP-1"]), (0, String::new(), String::new()));
    assert!(bus.log.has("GrantPackage org.example.sea key:signer audio"));
    let (code, out, err) = grant(&["--output", "DP-2"]);
    assert_eq!((code, out.as_str()), (0, ""), "{err}");
    assert!(err.contains("Asked in a desktop notification") && !err.contains("Error"), "{err}");
    let (code, _, err) = grant(&["-o", "DP-3"]);
    assert!(code == 1 && err.contains("DP-3 does not show org.example.sea"), "{err}");
    let (code, out, _) = bus.ctl(&["input", "status"]);
    assert_eq!(code, 0);
    assert!(out.contains("DP-1 shows org.example.sea using audio, clicks (allowed: audio)\n"), "{out}");
    assert!(out.contains("DP-2 shows org.example.sea using audio, clicks (allowed: none)\n"), "{out}");
    assert!(out.contains("clicks reach the wallpaper on: DP-1\ntaking clicks now: none\n"), "{out}");
    assert_eq!(bus.ctl(&["input", "clicks", "on", "-o", "DP-2"]).0, 0);
    assert_eq!(bus.ctl(&["input", "clicks", "off"]).0, 0);
    assert!(bus.log.has("SetClicks [DP-2] true") && bus.log.has("SetClicks [] false"));
    assert_eq!(grant(&["-o", "DP-1"]).0, 0);
    let (code, _, err) = bus.ctl(&["input", "grant", "org.example.sea", "clicks", "-o", "DP-1"]);
    assert!(code == 0 && bus.log.has("GrantPackage org.example.sea key:signer clicks"), "{err}");
    assert!(out.contains("granted to some version or signer of org.example.sea: audio\n"), "{out}");
}
