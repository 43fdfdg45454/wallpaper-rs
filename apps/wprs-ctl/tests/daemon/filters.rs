use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn the_filter_command_sends_a_look_and_its_adjustments() {
    let bus = Bus::start().await;
    let (code, out, _) = bus.ctl(&["filter", "--list"]);
    assert!(code == 0 && out.lines().count() == 16 && out.contains("sepia\n"), "{out}");
    let (code, _, err) = bus.ctl(&["filter", "warm", "--strength", "0.5", "--exposure", "-1", "-o", "DP-1"]);
    assert_eq!(code, 0, "{err}");
    let sent = r#"SetFilter [DP-1] {"look":"warm","strength":0.5,"exposure":-1.0,"contrast":0.0"#;
    assert!(bus.log.has(sent));
    assert_eq!(bus.ctl(&["filter", "none"]).0, 0);
    assert!(bus.log.has(r#"SetFilter [] {"look":"none","strength":1.0"#));
    let (code, _, err) = bus.ctl(&["filter", "teal"]);
    assert!(code != 0 && err.contains("unknown look 'teal'"), "{err}");
    let (code, _, err) = bus.ctl(&["filter", "mono", "--hue", "400"]);
    assert!(code != 0 && err.contains("hue takes -180 to 180"), "{err}");
}
