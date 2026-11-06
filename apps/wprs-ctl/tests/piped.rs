use std::io::Read;
use std::process::{Command, Stdio};

#[test]
fn a_reader_that_stops_early_leaves_a_successful_exit() {
    for _ in 0..5 {
        let mut child = Command::new(env!("CARGO_BIN_EXE_wprs-ctl"))
            .args(["scene", "schema"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut first = [0u8; 16];
        child.stdout.take().unwrap().read_exact(&mut first).unwrap();
        let out = child.wait_with_output().unwrap();
        let said = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success() && !said.contains("panicked"), "{:?} {said}", out.status);
    }
}
