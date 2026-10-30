use super::*;

#[test]
fn trials_of_processes_in_other_pid_namespaces_are_alive_until_they_end() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let apart =
        Command::new("unshare").args(["-Urpf", "--mount-proc", "true"]).status().is_ok_and(|s| s.success());
    if !apart {
        eprintln!("unshare is not available; the other processes share this pid namespace");
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stopped-effects");
    keep_strikes(&path);
    trying(0x41).unwrap();
    let mut settled = spawn(&format!("settle {} 99", path.display()), apart);
    strike(&[]).unwrap();
    assert!(!lines(&path).iter().any(|l| l.contains(" 1 ")), "{:?}", lines(&path));
    drop(settled.stdin.take());
    assert!(settled.wait().unwrap().success());
    let only = lines(&path);
    assert!(only.len() == 1 && only[0].starts_with(&format!("{:016x} trying ", 0x41)), "{only:?}");
    let mut killed = spawn(&format!("hold {} 98", path.display()), apart);
    killed.kill().unwrap();
    killed.wait().unwrap();
    let gone = |l: &String| l.starts_with(&format!("{:016x} 1 ", 0x98));
    let start = std::time::Instant::now();
    while strike(&[]).is_ok() && !lines(&path).iter().any(gone) && start.elapsed().as_secs() < 5 {
        std::thread::yield_now();
    }
    assert!(lines(&path).iter().any(|l| l.starts_with(&format!("{:016x} 1 ", 0x98))), "{:?}", lines(&path));
    tried(0x41);
    end_trials();
    let left = std::fs::read_dir(path.with_extension("trials")).unwrap().count();
    assert!(lines(&path).iter().all(|l| !l.contains("trying")) && left == 1, "{:?}", lines(&path));
}

#[test]
fn ending_trials_on_a_clean_exit_leaves_no_strike() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stopped-effects");
    keep_strikes(&path);
    trying(0x31).unwrap();
    trying(0x32).unwrap();
    end_trials();
    assert!(lines(&path).is_empty(), "{:?}", lines(&path));
    trying(0x33).unwrap();
    assert_eq!(lines(&path).len(), 1);
    end_trials();
    tried(0x33);
    assert!(lines(&path).is_empty());
}
