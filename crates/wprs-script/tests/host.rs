use std::process::{Command, ExitCode};
use std::time::{Duration, Instant};
use wprs_scene::ScriptOutputs;
use wprs_script::{Error, Frame, Host, Setup};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("--script-host") => {
            return wprs_script::serve().map_or(ExitCode::FAILURE, |()| ExitCode::SUCCESS);
        }
        Some("--sandbox-probe") => return probe(),
        Some("--hang") => loop {
            std::thread::sleep(Duration::from_secs(60));
        },
        _ => {}
    }
    let tests: [(&str, fn()); 6] = [
        ("scripts_tell_the_local_time_of_the_computer", scripts_tell_the_local_time_of_the_computer),
        ("the_sandbox_blocks_files_network_and_processes", the_sandbox_blocks_files_network_and_processes),
        ("the_host_runs_scripts_in_another_process", the_host_runs_scripts_in_another_process),
        ("runaway_scripts_in_the_host_are_stopped", runaway_scripts_in_the_host_are_stopped),
        ("scripts_that_do_not_load_are_disabled", scripts_that_do_not_load_are_disabled),
        ("a_host_that_hangs_is_killed_and_then_disabled", a_host_that_hangs_is_killed_and_then_disabled),
    ];
    for (name, test) in tests {
        test();
        println!("test {name} ... ok");
    }
    ExitCode::SUCCESS
}

fn probe() -> ExitCode {
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let (parent, kept) = (std::os::unix::process::parent_id(), std::fs::File::open(manifest).unwrap());
    let kept = std::os::fd::IntoRawFd::into_raw_fd(kept);
    wprs_script::sandbox().expect("the sandbox applies");
    let file = std::fs::File::open(manifest).is_err();
    let written = std::fs::write("wprs-sandbox-probe", b"x").is_err();
    let network = std::net::TcpListener::bind("127.0.0.1:0").is_err();
    let process = Command::new("true").status().is_err();
    let signal = unsafe { libc::syscall(libc::SYS_tgkill, parent, parent, 0) } != 0;
    let closed = unsafe { libc::read(kept, [0u8; 1].as_mut_ptr().cast(), 1) } < 0;
    let memory = vec![1u8; 16 << 20].iter().all(|&b| b == 1);
    let all = [file, written, network, process, signal, closed, memory];
    println!(
        "file {file} write {written} network {network} process {process} signal {signal} closed {closed}"
    );
    if all.iter().all(|&ok| ok) { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

fn host(args: &[&str], source: &str) -> Host {
    let setup = Setup { seed: 3, locale: "es".into(), ..Setup::plain(&[("main.js".into(), source.into())]) };
    Host::new(std::env::current_exe().unwrap(), args, &setup)
}

fn drive(host: &mut Host, until: impl Fn(&Result<Option<ScriptOutputs>, Error>) -> bool) {
    let start = Instant::now();
    for step in 0.. {
        let wall = 20_755.0 * 86_400.0 + 43_200.0;
        let frame = Frame { dt: 0.016, wall, zone: "<+0300>-03:00:00", ..Frame::at(step as f64) };
        let call = Instant::now();
        let result = host.update(&frame);
        assert!(call.elapsed() < Duration::from_millis(100), "an update took {:?}", call.elapsed());
        if until(&result) {
            return;
        }
        assert!(start.elapsed() < Duration::from_secs(10), "gave up at {result:?}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn the_sandbox_blocks_files_network_and_processes() {
    let out = Command::new(std::env::current_exe().unwrap()).arg("--sandbox-probe").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
}

fn the_host_runs_scripts_in_another_process() {
    let source = "import { engine, scene } from 'wprs'; const a = scene.layer('a'); \
        export function update({ time }) { a.rotation = time; a.text = engine.locale; }";
    let mut host = host(&["--script-host"], source);
    drive(&mut host, |r| {
        let turned = |o: &ScriptOutputs| {
            o.layers
                .get("a")
                .is_some_and(|a| a.rotation.is_some_and(|r| r > 0.0) && a.text.as_deref() == Some("es"))
        };
        r.as_ref().is_ok_and(|o| o.as_ref().is_some_and(turned))
    });
    let (frames, replies) = host.progress();
    assert!(frames >= 2 && replies + 1 == frames, "{frames} frames, {replies} replies");
}

fn scripts_tell_the_local_time_of_the_computer() {
    let source = "import { engine, scene, time } from 'wprs'; const a = scene.layer('a'); \
        export function update() { const d = new Date(Date.UTC(2026, 9, 29, 9)); \
        a.text = `${d.getHours()} ${time.format('EEEE HH:mm')} ${time.format('H', d)} ${engine.daytime}`; }";
    let mut host = host(&["--script-host"], source);
    drive(&mut host, |r| {
        let told = |o: &ScriptOutputs| o.layers.get("a").and_then(|a| a.text.clone());
        let text = r.as_ref().ok().and_then(|o| o.as_ref().and_then(told));
        text.is_some_and(|t| t == "12 jueves 12:00 12 0.5")
    });
}

fn runaway_scripts_in_the_host_are_stopped() {
    let mut host = host(&["--script-host"], "export function update() { for (;;) {} }");
    drive(&mut host, |r| matches!(r, Err(Error::Timeout(_))));
    drive(&mut host, |r| matches!(r, Err(Error::Disabled)));
}

fn scripts_that_do_not_load_are_disabled() {
    let mut broken = host(&["--script-host"], "export function update( {");
    let stopped =
        |o: &ScriptOutputs| o.stopped.iter().any(|s| s.starts_with("main.js: ") && s.contains("main.js:1:"));
    drive(&mut broken, |r| r.as_ref().is_ok_and(|o| o.as_ref().is_some_and(stopped)));
    let mut big = host(&["--script-host"], &" ".repeat((1 << 20) + 1));
    drive(&mut big, |r| matches!(r, Err(Error::Script(e)) if e.contains("1 MiB")));
    drive(&mut big, |r| matches!(r, Err(Error::Disabled)));
}

fn a_host_that_hangs_is_killed_and_then_disabled() {
    let mut host = host(&["--hang"], &" ".repeat(1 << 20));
    let start = Instant::now();
    drive(&mut host, |r| matches!(r, Err(Error::Host(_))));
    assert!(start.elapsed() >= Duration::from_millis(250));
    drive(&mut host, |r| matches!(r, Err(Error::Disabled)));
    assert!(matches!(drive_once(&mut host), Err(Error::Disabled)));
}

fn drive_once(host: &mut Host) -> Result<(), Error> {
    host.update(&Frame::at(0.0)).map(drop)
}
