use std::path::PathBuf;
use std::sync::OnceLock;

const NAME: &str = "wprs-web";

static PROBED: OnceLock<Result<String, String>> = OnceLock::new();

pub fn helper() -> Option<PathBuf> {
    let given = std::env::var_os("WPRS_WEB").map(PathBuf::from);
    let beside = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join(NAME)));
    let path = std::env::var_os("PATH").unwrap_or_default();
    let searched = std::env::split_paths(&path).map(|dir| dir.join(NAME));
    given.into_iter().chain(beside).chain(searched).find(|p| p.is_file())
}

fn probe() -> Result<String, String> {
    let Some(path) = helper() else { return Err(crate::words::web_missing()) };
    let output = std::process::Command::new(&path).arg("--check").stdin(std::process::Stdio::null()).output();
    let output = output.map_err(|e| crate::words::web_broken(&e.to_string()))?;
    let said = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    match output.status.success() {
        true => Ok(said),
        false if said.contains("libWPEWebKit") => Err(crate::words::web_engine()),
        false if said.contains("older than") => Err(crate::words::web_old(&said)),
        false => Err(crate::words::web_broken(&said)),
    }
}

pub fn available() -> Result<String, String> {
    PROBED.get_or_init(probe).clone()
}

pub fn unavailable() -> Option<String> {
    available().err()
}
