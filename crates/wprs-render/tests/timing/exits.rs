use super::hostile::{entry, renderer, scene, show, unchecked};
use super::{book, one, slow_build};
use std::os::unix::process::ExitStatusExt;
use std::time::Duration;

const CHILD: &str = "WPRS_EXIT_CHILD";

#[test]
fn exiting_while_a_package_shader_builds_never_crashes() {
    if std::env::var_os(CHILD).is_some() {
        let Some(r) = renderer() else { std::process::exit(0) };
        book("exit");
        wprs_render::set_build_timeout(Duration::from_millis(100));
        let mut s = scene(&[("slow", entry("return tap(in.uv);"))]);
        unchecked(&mut s, "slow", &slow_build());
        let shown = show(r, s).unwrap();
        assert!(shown.view.left_out().iter().any(|l| l.contains("to build")));
        wprs_render::finish_builds(0);
        std::process::exit(0);
    }
    let _one = one();
    if renderer().is_none() {
        return;
    }
    let name = "exits::exiting_while_a_package_shader_builds_never_crashes";
    for _ in 0..3 {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture", "--test-threads=1"])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert_eq!((status.code(), status.signal()), (Some(0), None), "{status:?}");
    }
}
