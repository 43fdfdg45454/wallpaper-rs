use crate::depthwork::{Report, sha};
use anyhow::{Context, Result, anyhow, bail};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::Duration;
use wprs_core::edit::{self, Depth};
use wprs_depth::{Places, Stage};

pub fn generate(image: &Path, input: u32, stop: &AtomicBool, report: Report<'_>) -> Result<()> {
    let mut child = Command::new(std::env::current_exe()?)
        .arg("--depth")
        .arg(image)
        .arg(input.to_string())
        .env(wprs_depth::TELEMETRY, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let lines = BufReader::new(child.stdout.take().context("no output from the depth process")?).lines();
    let (child, over) = (Mutex::new(child), AtomicBool::new(false));
    let mut result = Err(anyhow!("the depth process stopped"));
    std::thread::scope(|s| {
        s.spawn(|| {
            while !over.load(Relaxed) {
                if stop.load(Relaxed) {
                    return drop(child.lock().map(|mut c| c.kill()));
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        for line in lines.map_while(Result::ok) {
            match line.split_once(' ') {
                Some(("stage", stage)) => report(stage, -1.0),
                Some(("done", _)) => result = Ok(()),
                Some(("error", message)) => result = Err(anyhow!("{message}")),
                _ => {}
            }
        }
        over.store(true, Relaxed);
    });
    child.into_inner().map_err(|_| anyhow!("the depth process is lost"))?.wait()?;
    if stop.load(Relaxed) {
        bail!("cancelled");
    }
    result
}

pub fn child(args: &[String]) -> ExitCode {
    let (Some(image), input) = (args.first().map(PathBuf::from), args.get(1).and_then(|a| a.parse().ok()))
    else {
        return ExitCode::FAILURE;
    };
    wprs_depth::background();
    let made = (|| {
        let edit = edit::load(&image, &edit::dir()).ok().flatten().map(|(_, e)| e).unwrap_or_default();
        let depth = edit.depth.clone().unwrap_or_default();
        let depth = Depth { input: input.unwrap_or(depth.input), ..depth };
        let mut names = |stage| match stage {
            Stage::Loading => println!("stage loading"),
            Stage::Estimating => println!("stage estimating"),
            Stage::Saving => println!("stage saving"),
        };
        wprs_depth::generate(&Places::default(), (&image, &sha(&image, &edit)?), &depth, None, &mut names)
    })();
    match made {
        Ok(path) => println!("done {}", path.display()),
        Err(e) => {
            println!("error {e:#}");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
