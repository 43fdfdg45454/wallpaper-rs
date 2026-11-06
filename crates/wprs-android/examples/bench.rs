use wprs_android::host;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [files, wpak, rest @ ..] = &args[..] else {
        eprintln!(
            "usage: bench <files dir> <file.wpak> [--level q0|q1|q2|q3|auto|fitted] [--frames N] [--paced SECONDS] [--fps N] [--size WxH] [--passes] [--shot FILE.png]"
        );
        std::process::exit(2);
    };
    if let Err(e) = host::prepare(files) {
        eprintln!("the files dir cannot be used: {e}");
        std::process::exit(1);
    }
    let report = host::bench(wpak, rest);
    println!("{report}");
    if report.starts_with("{\"error\"") {
        std::process::exit(1);
    }
}
