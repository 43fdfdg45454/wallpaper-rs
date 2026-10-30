use super::*;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;

pub(super) fn failed_helpers_refuse_without_strikes(book: &Path) {
    let cases: [(&str, &str, Change); 4] = [
        ("abort", "the checker crashed with signal 6", |_| ()),
        ("huge", "the checker ran out of memory", |_| ()),
        ("hang", "the checker took longer than 1 seconds", |c| c.wall = Duration::from_secs(1)),
        ("spin", "the checker used more than 1 seconds of processor time", |c| c.cpu = 1),
    ];
    for (id, expected, change) in cases {
        checker(change);
        let runs = helper_runs();
        let (fault, took) = timed(|| Descriptor::packaged(id, &json(id), GOOD).unwrap_err());
        println!("{id}: {fault} after {took:?}");
        assert_eq!(fault, format!("its shader could not be checked: {expected}"));
        assert!(lines(book).is_empty(), "{id}: {:?}", lines(book));
        assert_eq!(lines(&book.with_file_name("checked-effects")).len(), 2);
        if id == "abort" {
            assert!(Descriptor::packaged(id, &json(id), GOOD).is_err());
            assert_eq!(helper_runs(), runs + 2, "a crash is not remembered");
        }
    }
    checker(|_| ());
}

pub(super) fn lower_hard_limits_of_the_parent_still_let_the_helper_check(book: &Path) {
    let me = std::env::current_exe().unwrap();
    let mut command = Command::new(me);
    command.arg("--limited").arg(book);
    unsafe {
        command.pre_exec(|| {
            let limits = [(libc::RLIMIT_AS, 900 << 20), (libc::RLIMIT_CPU, 4)];
            for (resource, most) in limits {
                let limit = libc::rlimit { rlim_cur: most, rlim_max: most };
                libc::setrlimit(resource, &limit);
            }
            Ok(())
        })
    };
    let out = command.output().unwrap();
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && said.trim() == "accepted",
        "{said}{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

pub(super) fn limited(book: &str) {
    keep_strikes(Path::new(book));
    check_out_of_process(true);
    let good = GOOD.replace("tap(in.uv)", "tap(in.uv).bgra");
    match Descriptor::packaged("h", &json("h"), &good) {
        Ok(_) => println!("accepted"),
        Err(e) => println!("{e}"),
    }
}

pub(super) fn a_process_killed_while_the_helper_runs_gets_no_strike(book: &Path) {
    let script = book.with_file_name("slow-checker");
    std::fs::write(&script, "#!/bin/sh\nsleep 30\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let me = std::env::current_exe().unwrap();
    for _ in 0..2 {
        let mut child = Command::new(&me).arg("--slow").arg(book).arg(&script).spawn().unwrap();
        std::thread::sleep(Duration::from_millis(700));
        child.kill().unwrap();
        child.wait().unwrap();
    }
    keep_strikes(book);
    assert!(lines(book).is_empty(), "{:?}", lines(book));
    set_checker(None);
    Descriptor::packaged("s", &json("s"), SLOW).unwrap();
    checker(|_| ());
}

pub(super) fn slow(book: &str, script: &str) {
    keep_strikes(Path::new(book));
    checker(|c| c.program = script.into());
    drop(Descriptor::packaged("s", &json("s"), SLOW));
}
